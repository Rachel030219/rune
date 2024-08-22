use std::collections::HashSet;

use sea_orm::prelude::*;
use sea_orm::ActiveValue;
use sea_orm::QueryOrder;

use crate::entities::{media_file_playlists, playlists};
use crate::get_groups;

use super::utils::CountByFirstLetter;

impl CountByFirstLetter for playlists::Entity {
    fn group_column() -> Self::Column {
        playlists::Column::Group
    }

    fn id_column() -> Self::Column {
        playlists::Column::Id
    }
}

get_groups!(
    get_playlists_groups,
    playlists,
    media_file_playlists,
    PlaylistId
);

/// Create a new playlist.
///
/// # Arguments
/// * `db` - A reference to the database connection.
/// * `name` - The name of the new playlist.
/// * `group` - The group to which the playlist belongs.
///
/// # Returns
/// * `Result<Model, Box<dyn std::error::Error>>` - The created playlist model or an error.
pub async fn create_playlist(
    db: &DatabaseConnection,
    name: String,
    group: String,
) -> Result<playlists::Model, Box<dyn std::error::Error>> {
    use playlists::ActiveModel;

    // Create a new playlist active model
    let new_playlist = ActiveModel {
        name: ActiveValue::Set(name),
        group: ActiveValue::Set(group),
        ..Default::default()
    };

    // Insert the new playlist into the database
    let playlist = new_playlist.insert(db).await?;

    Ok(playlist)
}

/// Update an existing playlist.
///
/// # Arguments
/// * `db` - A reference to the database connection.
/// * `playlist_id` - The ID of the playlist to update.
/// * `name` - The new name for the playlist.
/// * `group` - The new group for the playlist.
///
/// # Returns
/// * `Result<Model, Box<dyn std::error::Error>>` - The updated playlist model or an error.
pub async fn update_playlist(
    db: &DatabaseConnection,
    playlist_id: i32,
    name: Option<String>,
    group: Option<String>,
) -> Result<playlists::Model, Box<dyn std::error::Error>> {
    use playlists::Entity as PlaylistEntity;

    // Find the playlist by ID
    let mut playlist: playlists::ActiveModel = PlaylistEntity::find_by_id(playlist_id)
        .one(db)
        .await?
        .ok_or("Playlist not found")?
        .into();

    // Update the fields if provided
    if let Some(name) = name {
        playlist.name = ActiveValue::Set(name);
    }
    if let Some(group) = group {
        playlist.group = ActiveValue::Set(group);
    }

    // Update the playlist in the database
    let updated_playlist = playlist.update(db).await?;

    Ok(updated_playlist)
}

/// Check for duplicate items in a playlist.
///
/// # Arguments
/// * `db` - A reference to the database connection.
/// * `playlist_id` - The ID of the playlist to check.
///
/// # Returns
/// * `Result<Vec<i32>, Box<dyn std::error::Error>>` - A vector of duplicate media file IDs or an error.
pub async fn check_items_in_playlist(
    db: &DatabaseConnection,
    playlist_id: i32,
    media_file_ids: Vec<i32>,
) -> Result<Vec<i32>, Box<dyn std::error::Error>> {
    use media_file_playlists::Entity as MediaFilePlaylistEntity;

    let items = MediaFilePlaylistEntity::find()
        .filter(media_file_playlists::Column::PlaylistId.eq(playlist_id))
        .filter(media_file_playlists::Column::MediaFileId.is_in(media_file_ids.clone()))
        .all(db)
        .await?;

    let seen: HashSet<_> = media_file_ids.into_iter().collect();
    let mut duplicates = vec![];

    for item in items {
        if seen.contains(&item.media_file_id) {
            duplicates.push(item.media_file_id);
        }
    }

    Ok(duplicates)
}

/// Add a media file to a playlist.
///
/// # Arguments
/// * `db` - A reference to the database connection.
/// * `playlist_id` - The ID of the playlist to add the item to.
/// * `media_file_id` - The ID of the media file to add.
/// * `position` - The position of the media file in the playlist.
///
/// # Returns
/// * `Result<Model, Box<dyn std::error::Error>>` - The created media file playlist model or an error.
pub async fn add_item_to_playlist(
    db: &DatabaseConnection,
    playlist_id: i32,
    media_file_id: i32,
    position: i32,
) -> Result<media_file_playlists::Model, Box<dyn std::error::Error>> {
    use media_file_playlists::ActiveModel;

    // Create a new media file playlist active model
    let new_media_file_playlist = ActiveModel {
        playlist_id: ActiveValue::Set(playlist_id),
        media_file_id: ActiveValue::Set(media_file_id),
        position: ActiveValue::Set(position),
        ..Default::default()
    };

    // Insert the new media file playlist into the database
    let media_file_playlist = new_media_file_playlist.insert(db).await?;

    Ok(media_file_playlist)
}

/// Add a media file to a playlist.
///
/// # Arguments
/// * `db` - A reference to the database connection.
/// * `playlist_id` - The ID of the playlist to add the media file to.
/// * `media_file_id` - The ID of the media file to add.
///
/// # Returns
/// * `Result<Model, Box<dyn std::error::Error>>` - The created media file playlist model or an error.
pub async fn add_media_file_to_playlist(
    db: &DatabaseConnection,
    playlist_id: i32,
    media_file_id: i32,
) -> Result<media_file_playlists::Model, Box<dyn std::error::Error>> {
    use media_file_playlists::Entity as MediaFilePlaylistEntity;

    // Get the current maximum position in the playlist
    let max_position = MediaFilePlaylistEntity::find()
        .filter(media_file_playlists::Column::PlaylistId.eq(playlist_id))
        .order_by_desc(media_file_playlists::Column::Position)
        .one(db)
        .await?
        .map_or(0, |item| item.position);

    // Create a new media file playlist active model
    let new_media_file_playlist = media_file_playlists::ActiveModel {
        playlist_id: ActiveValue::Set(playlist_id),
        media_file_id: ActiveValue::Set(media_file_id),
        position: ActiveValue::Set(max_position + 1),
        ..Default::default()
    };

    // Insert the new media file playlist into the database
    let media_file_playlist = new_media_file_playlist.insert(db).await?;

    Ok(media_file_playlist)
}

/// Reorder a media file in a playlist.
///
/// # Arguments
/// * `db` - A reference to the database connection.
/// * `playlist_id` - The ID of the playlist containing the item to reorder.
/// * `media_file_id` - The ID of the media file to reorder.
/// * `new_position` - The new position for the media file.
///
/// # Returns
/// * `Result<(), Box<dyn std::error::Error>>` - An empty result or an error.
pub async fn reorder_playlist_item_position(
    db: &DatabaseConnection,
    playlist_id: i32,
    media_file_id: i32,
    new_position: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    use media_file_playlists::Entity as MediaFilePlaylistEntity;

    // Find the media file playlist item
    let mut item: media_file_playlists::ActiveModel = MediaFilePlaylistEntity::find()
        .filter(media_file_playlists::Column::PlaylistId.eq(playlist_id))
        .filter(media_file_playlists::Column::MediaFileId.eq(media_file_id))
        .one(db)
        .await?
        .ok_or("Media file not found in playlist")?
        .into();

    // Update the position
    item.position = ActiveValue::Set(new_position);

    // Update the media file playlist item in the database
    item.update(db).await?;

    Ok(())
}
