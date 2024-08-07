use dunce::canonicalize;
use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::path::Path;

use metadata::cover_art::extract_cover_art_binary;

use crate::entities::{media_cover_art, media_files};

pub async fn get_cover_art(
    db: &DatabaseConnection,
    lib_path: &str,
    file_id: i32,
) -> Result<Option<Vec<u8>>, sea_orm::DbErr> {
    // Query file information
    let file: Option<media_files::Model> = media_files::Entity::find_by_id(file_id).one(db).await?;

    if let Some(file) = file {
        if let Some(cover_art_id) = file.cover_art_id {
            // If cover_art_id already exists, directly retrieve the cover art from the database
            let cover_art = media_cover_art::Entity::find_by_id(cover_art_id)
                .one(db)
                .await?;
            Ok(Some(cover_art.unwrap().binary))
        } else {
            let file_path = canonicalize(
                Path::new(lib_path)
                    .join(file.directory.clone())
                    .join(file.file_name.clone()),
            )
            .unwrap();
            // If cover_art_id is empty, it means the file has not been checked before
            if let Some(cover_art) = extract_cover_art_binary(&file_path) {
                // Check if there is a file with the same CRC in the database
                let existing_cover_art = media_cover_art::Entity::find()
                    .filter(media_cover_art::Column::FileHash.eq(cover_art.crc.clone()))
                    .one(db)
                    .await?;

                if let Some(existing_cover_art) = existing_cover_art {
                    // If there is a file with the same CRC, update the file's cover_art_id
                    let mut file_active_model: media_files::ActiveModel = file.into();
                    file_active_model.cover_art_id = ActiveValue::Set(Some(existing_cover_art.id));
                    media_files::Entity::update(file_active_model)
                        .exec(db)
                        .await?;

                    Ok(Some(existing_cover_art.binary))
                } else {
                    // If there is no file with the same CRC, store the cover art in the database and update the file's cover_art_id
                    let new_cover_art = media_cover_art::ActiveModel {
                        id: ActiveValue::NotSet,
                        file_hash: ActiveValue::Set(cover_art.crc.clone()),
                        binary: ActiveValue::Set(cover_art.data.clone()),
                    };

                    let insert_result = media_cover_art::Entity::insert(new_cover_art)
                        .exec(db)
                        .await?;
                    let new_cover_art_id = insert_result.last_insert_id;

                    let mut file_active_model: media_files::ActiveModel = file.into();
                    file_active_model.cover_art_id = ActiveValue::Set(Some(new_cover_art_id));
                    media_files::Entity::update(file_active_model)
                        .exec(db)
                        .await?;

                    Ok(Some(cover_art.data))
                }
            } else {
                // If the audio file has no cover art, check if there is a magic value with an empty CRC in the database
                let magic_cover_art = media_cover_art::Entity::find()
                    .filter(media_cover_art::Column::FileHash.eq(String::new()))
                    .one(db)
                    .await?;

                if let Some(magic_cover_art) = magic_cover_art {
                    // If the magic value exists, update the file's cover_art_id
                    let mut file_active_model: media_files::ActiveModel = file.into();
                    file_active_model.cover_art_id = ActiveValue::Set(Some(magic_cover_art.id));
                    media_files::Entity::update(file_active_model)
                        .exec(db)
                        .await?;

                    Ok(Some(magic_cover_art.binary))
                } else {
                    // If the magic value does not exist, create one and update the file's cover_art_id
                    let new_magic_cover_art = media_cover_art::ActiveModel {
                        id: ActiveValue::NotSet,
                        file_hash: ActiveValue::Set(String::new()),
                        binary: ActiveValue::Set(Vec::new()),
                    };

                    let insert_result = media_cover_art::Entity::insert(new_magic_cover_art)
                        .exec(db)
                        .await?;
                    let new_magic_cover_art_id = insert_result.last_insert_id;

                    let mut file_active_model: media_files::ActiveModel = file.into();
                    file_active_model.cover_art_id = ActiveValue::Set(Some(new_magic_cover_art_id));
                    media_files::Entity::update(file_active_model)
                        .exec(db)
                        .await?;

                    Ok(Some(Vec::new()))
                }
            }
        }
    } else {
        Ok(None)
    }
}
