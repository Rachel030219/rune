use std::path::Path;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use log::{error, info};
use paste::paste;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue, QuerySelect, TransactionTrait};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use seq_macro::seq;
use tokio::task;
use tokio_util::sync::CancellationToken;

use analysis::analysis::{analyze_audio, normalize_analysis_result, NormalizedAnalysisResult};

use crate::entities::{media_analysis, media_files};

use super::utils::DatabaseExecutor;

pub fn empty_progress_callback(_processed: usize, _total: usize) {}

/// Analyze the audio library by reading existing files, checking if they have been analyzed,
/// and performing audio analysis if not. The function uses cursor pagination to process files
/// in batches for memory efficiency and utilizes multi-core parallelism for faster processing.
/// The analysis results are normalized before being stored in the database.
///
/// # Arguments
/// * `main_db` - A reference to the database connection.
/// * `lib_path` - The root path for the audio files.
/// * `batch_size` - The number of files to process in each batch.
/// * `progress_callback` - A callback function to report progress.
/// * `cancel_token` - An optional cancellation token to support task cancellation.
///
/// # Returns
/// * `Result<(), sea_orm::DbErr>` - A result indicating success or failure.
pub async fn analysis_audio_library<F>(
    main_db: &DatabaseConnection,
    lib_path: &Path,
    batch_size: usize,
    progress_callback: F,
    cancel_token: Option<CancellationToken>,
) -> Result<usize>
where
    F: Fn(usize, usize) + Send + Sync,
{
    info!(
        "Starting audio library analysis with batch size: {}",
        batch_size
    );

    let existed_ids: Vec<i32> = media_analysis::Entity::find()
        .select_only()
        .column(media_analysis::Column::FileId)
        .distinct()
        .into_tuple::<i32>()
        .all(main_db)
        .await?;

    info!("Anready analysed files: {}", existed_ids.len());

    let mut cursor = media_files::Entity::find()
        .filter(media_files::Column::Id.is_not_in(existed_ids))
        .cursor_by(media_files::Column::Id);

    // Calculate the total number of tasks
    let total_tasks = media_files::Entity::find().count(main_db).await? as usize;

    let (tx, rx) = async_channel::bounded(batch_size);
    let mut total_processed = 0;

    // Producer task: fetch batches of files and send them to the consumer
    let producer = async {
        loop {
            // Check for cancellation
            if let Some(ref token) = cancel_token {
                if token.is_cancelled() {
                    info!("Cancellation requested. Exiting producer loop.");
                    break;
                }
            }

            // Fetch the next batch of files
            let files: Vec<media_files::Model> = cursor
                .first(batch_size.try_into().unwrap())
                .all(main_db)
                .await?;

            if files.is_empty() {
                info!("No more files to process. Exiting loop.");
                break;
            }

            for file in &files {
                tx.send(file.clone()).await.unwrap();
            }

            // Move the cursor to the next batch
            if let Some(last_file) = files.last() {
                info!("Moving cursor after file ID: {}", last_file.id);
                cursor.after(last_file.id);
            } else {
                break;
            }
        }

        drop(tx); // Close the channel to signal consumers to stop
        Ok::<(), sea_orm::DbErr>(())
    };

    // Consumer task: process files as they are received
    let consumer = async {
        let mut tasks = Vec::new();

        while let Ok(file) = rx.recv().await {
            // Check for cancellation
            if let Some(ref token) = cancel_token {
                if token.is_cancelled() {
                    info!("Cancellation requested. Exiting consumer loop.");
                    break;
                }
            }

            let lib_path = lib_path.to_path_buf();
            let file_id = file.id;

            let task = task::spawn(async move {
                info!("Processing file with ID: {}", file_id);
                (file_id, analysis_file(&file, &lib_path).await)
            });

            tasks.push(task);

            // Process tasks in parallel up to the batch size
            if tasks.len() >= batch_size {
                let task_count = tasks.len();
                let results: Vec<_> = stream::iter(tasks)
                    .buffer_unordered(batch_size)
                    .collect()
                    .await;
                tasks = Vec::new();

                let txn = main_db.begin().await?;

                for result in results {
                    match result {
                        Ok((file_id, x)) => match x {
                            Ok(x) => insert_analysis_result(&txn, file_id, x).await?,
                            Err(e) => error!("Error processing file: {:?}", e),
                        },
                        Err(e) => error!("Error processing file: {:?}", e),
                    }
                }

                txn.commit().await?;

                // Update progress
                total_processed += task_count;
                progress_callback(total_processed, total_tasks);
            }
        }

        // Process remaining tasks
        if !tasks.is_empty() {
            let task_count = tasks.len();
            let results: Vec<_> = stream::iter(tasks)
                .buffer_unordered(batch_size)
                .collect()
                .await;
            for result in results {
                if let Err(e) = result {
                    error!("Error processing file: {:?}", e);
                }
            }

            // Update progress for remaining tasks
            total_processed += task_count;
            progress_callback(total_processed, total_tasks);
        }

        Ok::<(), sea_orm::DbErr>(())
    };

    // Run producer and consumer concurrently
    let (producer_result, consumer_result) = futures::join!(producer, consumer);

    producer_result?;
    consumer_result?;

    info!("Audio library analysis completed.");
    Ok(total_tasks)
}

/// Process a file if it has not been analyzed yet. Perform audio analysis and store the results
/// in the database.
///
/// # Arguments
/// * `db` - A reference to the database connection.
/// * `file` - A reference to the file model.
/// * `root_path` - The root path for the audio files.
async fn analysis_file(
    file: &media_files::Model,
    lib_path: &Path,
) -> Result<NormalizedAnalysisResult> {
    // Construct the full path to the file
    let file_path = lib_path.join(&file.directory).join(&file.file_name);

    // Perform audio analysis
    let analysis_result = analyze_audio(
        file_path.to_str().unwrap(),
        1024, // Example window size
        512,  // Example overlap size
    );

    // Normalize the analysis result
    Ok(normalize_analysis_result(&analysis_result?))
}

/// Insert the normalized analysis result into the database.
///
/// # Arguments
/// * `db` - A reference to the database connection.
/// * `file_id` - The ID of the file being analyzed.
/// * `result` - The normalized analysis result.
async fn insert_analysis_result<E>(
    db: &E,
    file_id: i32,
    result: NormalizedAnalysisResult,
) -> Result<(), sea_orm::DbErr>
where
    E: DatabaseExecutor + sea_orm::ConnectionTrait,
{
    let mut new_analysis = media_analysis::ActiveModel {
        file_id: ActiveValue::Set(file_id),
        rms: ActiveValue::Set(Some(result.raw.rms as f64)),
        zcr: ActiveValue::Set(Some(result.zcr as f64)),
        energy: ActiveValue::Set(Some(result.energy as f64)),
        spectral_centroid: ActiveValue::Set(Some(result.spectral_centroid as f64)),
        spectral_flatness: ActiveValue::Set(Some(result.spectral_flatness as f64)),
        spectral_slope: ActiveValue::Set(Some(result.spectral_slope as f64)),
        spectral_rolloff: ActiveValue::Set(Some(result.spectral_rolloff as f64)),
        spectral_spread: ActiveValue::Set(Some(result.spectral_spread as f64)),
        spectral_skewness: ActiveValue::Set(Some(result.spectral_skewness as f64)),
        spectral_kurtosis: ActiveValue::Set(Some(result.spectral_kurtosis as f64)),
        perceptual_spread: ActiveValue::Set(Some(result.raw.perceptual_spread as f64)),
        perceptual_sharpness: ActiveValue::Set(Some(result.raw.perceptual_sharpness as f64)),
        ..Default::default()
    };

    seq!(N in 0..12 {
        new_analysis.chroma~N = ActiveValue::Set(Some(result.chroma[N] as f64));
    });

    seq!(N in 0..24 {
        new_analysis.perceptual_loudness~N = ActiveValue::Set(Some(result.raw.perceptual_loudness[N] as f64));
    });

    seq!(N in 0..13 {
        new_analysis.mfcc~N = ActiveValue::Set(Some(result.raw.mfcc[N] as f64));
    });

    media_analysis::Entity::insert(new_analysis)
        .exec(db)
        .await?;

    Ok(())
}

/// Struct to store mean values of analysis results.
#[derive(Debug)]
pub struct AggregatedAnalysisResult {
    pub rms: f64,
    pub zcr: f64,
    pub energy: f64,
    pub spectral_centroid: f64,
    pub spectral_flatness: f64,
    pub spectral_slope: f64,
    pub spectral_rolloff: f64,
    pub spectral_spread: f64,
    pub spectral_skewness: f64,
    pub spectral_kurtosis: f64,
    pub chroma: [f64; 12],
    pub perceptual_spread: f64,
    pub perceptual_sharpness: f64,
    pub perceptual_loudness: [f64; 24],
    pub mfcc: [f64; 13],
}

impl From<AggregatedAnalysisResult> for [f32; 61] {
    fn from(val: AggregatedAnalysisResult) -> Self {
        vec![
            val.rms,
            val.zcr,
            val.energy,
            val.spectral_centroid,
            val.spectral_flatness,
            val.spectral_slope,
            val.spectral_rolloff,
            val.spectral_spread,
            val.spectral_skewness,
            val.spectral_kurtosis,
            val.chroma[0],
            val.chroma[1],
            val.chroma[2],
            val.chroma[3],
            val.chroma[4],
            val.chroma[5],
            val.chroma[6],
            val.chroma[7],
            val.chroma[8],
            val.chroma[9],
            val.chroma[10],
            val.chroma[11],
            val.perceptual_spread,
            val.perceptual_sharpness,
            val.perceptual_loudness[0],
            val.perceptual_loudness[1],
            val.perceptual_loudness[2],
            val.perceptual_loudness[3],
            val.perceptual_loudness[4],
            val.perceptual_loudness[5],
            val.perceptual_loudness[6],
            val.perceptual_loudness[7],
            val.perceptual_loudness[8],
            val.perceptual_loudness[9],
            val.perceptual_loudness[10],
            val.perceptual_loudness[11],
            val.perceptual_loudness[12],
            val.perceptual_loudness[13],
            val.perceptual_loudness[14],
            val.perceptual_loudness[15],
            val.perceptual_loudness[16],
            val.perceptual_loudness[17],
            val.perceptual_loudness[18],
            val.perceptual_loudness[19],
            val.perceptual_loudness[20],
            val.perceptual_loudness[21],
            val.perceptual_loudness[22],
            val.perceptual_loudness[23],
            val.mfcc[0],
            val.mfcc[1],
            val.mfcc[2],
            val.mfcc[3],
            val.mfcc[4],
            val.mfcc[5],
            val.mfcc[6],
            val.mfcc[7],
            val.mfcc[8],
            val.mfcc[9],
            val.mfcc[10],
            val.mfcc[11],
            val.mfcc[12],
        ]
        .into_iter()
        .map(|x| x as f32)
        .collect::<Vec<f32>>()
        .try_into()
        .expect("Expected a Vec of length 61")
    }
}

/// Macro to process individual fields by updating their sum and count.
macro_rules! process_field {
    ($sum:expr, $count:expr, $result:expr, $field:ident) => {
        if let Some(value) = $result.$field {
            $sum.$field += value;
            $count.$field += 1.0;
        }
    };
}

/// Macro to process array fields by updating their sum and count.
macro_rules! process_array {
    ($sum:expr, $count:expr, $result:expr, $field_prefix:ident, $size:expr) => {
        seq!(N in 0..$size {
            paste! {
                if let Some(value) = $result.[<$field_prefix N>] {
                    $sum.[<$field_prefix>][N] += value;
                    $count.[<$field_prefix>][N] += 1.0;
                }
            }
        });
    };
}

/// Macro to calculate the mean of individual fields.
macro_rules! calculate_mean {
    ($sum:expr, $count:expr, $field:ident) => {
        if $count.$field > 0.0 {
            $sum.$field / $count.$field
        } else {
            0.0
        }
    };
}

/// Macro to calculate the mean of array fields.
macro_rules! calculate_array_mean {
    ($sum:expr, $count:expr, $field:ident, $size:expr) => {{
        let mut result = [0.0; $size];
        for i in 0..$size {
            result[i] = if $count.$field[i] > 0.0 {
                $sum.$field[i] / $count.$field[i]
            } else {
                0.0
            };
        }
        result
    }};
}

/// Computes the centralized analysis result from the database.
///
/// This function retrieves analysis results based on specified file IDs,
/// sums the parameters, and calculates averages while handling potential `None` values.
///
/// # Arguments
///
/// * `db` - A reference to the database connection.
/// * `file_ids` - A vector of file IDs to filter the analysis results.
///
/// # Returns
///
/// * `AnalysisResultMean` - A struct containing the mean values of the analysis results.
///
/// # Example
///
/// ```rust
/// let db: DatabaseConnection = ...;
/// let file_ids = vec![1, 2, 3];
/// let result = get_centralized_analysis_result(&db, file_ids).await;
/// println!("{:?}", result);
/// ```
pub async fn get_centralized_analysis_result(
    db: &DatabaseConnection,
    file_ids: Vec<i32>,
) -> AggregatedAnalysisResult {
    let analysis_results = media_analysis::Entity::find()
        .filter(media_analysis::Column::FileId.is_in(file_ids))
        .all(db)
        .await
        .unwrap();

    let mut sum = AggregatedAnalysisResult {
        rms: 0.0,
        zcr: 0.0,
        energy: 0.0,
        spectral_centroid: 0.0,
        spectral_flatness: 0.0,
        spectral_slope: 0.0,
        spectral_rolloff: 0.0,
        spectral_spread: 0.0,
        spectral_skewness: 0.0,
        spectral_kurtosis: 0.0,
        chroma: [0.0; 12],
        perceptual_spread: 0.0,
        perceptual_sharpness: 0.0,
        perceptual_loudness: [0.0; 24],
        mfcc: [0.0; 13],
    };

    let mut count = AggregatedAnalysisResult {
        rms: 0.0,
        zcr: 0.0,
        energy: 0.0,
        spectral_centroid: 0.0,
        spectral_flatness: 0.0,
        spectral_slope: 0.0,
        spectral_rolloff: 0.0,
        spectral_spread: 0.0,
        spectral_skewness: 0.0,
        spectral_kurtosis: 0.0,
        chroma: [0.0; 12],
        perceptual_spread: 0.0,
        perceptual_sharpness: 0.0,
        perceptual_loudness: [0.0; 24],
        mfcc: [0.0; 13],
    };

    for result in analysis_results {
        process_field!(sum, count, result, rms);
        process_field!(sum, count, result, zcr);
        process_field!(sum, count, result, energy);
        process_field!(sum, count, result, spectral_centroid);
        process_field!(sum, count, result, spectral_flatness);
        process_field!(sum, count, result, spectral_slope);
        process_field!(sum, count, result, spectral_rolloff);
        process_field!(sum, count, result, spectral_spread);
        process_field!(sum, count, result, spectral_skewness);
        process_field!(sum, count, result, spectral_kurtosis);
        process_field!(sum, count, result, perceptual_spread);
        process_field!(sum, count, result, perceptual_sharpness);

        process_array!(sum, count, result, perceptual_loudness, 24);
        process_array!(sum, count, result, mfcc, 13);
        process_array!(sum, count, result, chroma, 12);
    }

    AggregatedAnalysisResult {
        rms: calculate_mean!(sum, count, rms),
        zcr: calculate_mean!(sum, count, zcr),
        energy: calculate_mean!(sum, count, energy),
        spectral_centroid: calculate_mean!(sum, count, spectral_centroid),
        spectral_flatness: calculate_mean!(sum, count, spectral_flatness),
        spectral_slope: calculate_mean!(sum, count, spectral_slope),
        spectral_rolloff: calculate_mean!(sum, count, spectral_rolloff),
        spectral_spread: calculate_mean!(sum, count, spectral_spread),
        spectral_skewness: calculate_mean!(sum, count, spectral_skewness),
        spectral_kurtosis: calculate_mean!(sum, count, spectral_kurtosis),
        perceptual_spread: calculate_mean!(sum, count, perceptual_spread),
        perceptual_sharpness: calculate_mean!(sum, count, perceptual_sharpness),
        chroma: calculate_array_mean!(sum, count, chroma, 12),
        perceptual_loudness: calculate_array_mean!(sum, count, perceptual_loudness, 24),
        mfcc: calculate_array_mean!(sum, count, mfcc, 13),
    }
}
