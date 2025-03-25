


use std::{str::FromStr, sync::Arc};

use axum::{extract::State, Json};
use sessionless::{Sessionless, Signature};

use crate::{config::AppState, storage::PubKeys};

use super::{DeleteUserRequest, Response};

// Deletes the user from storage and the public key + hash
pub async fn delete_user_handler(
    State(data): State<Arc<AppState>>,
    Json(body): Json<DeleteUserRequest>,
) -> Json<Response> {

    let message = format!("{}{}{}", body.timestamp, body.user_uuid, body.hash);
    let sessionless = Sessionless::new();

    let sig = match Signature::from_str(body.signature.as_str()) {
        Ok(s) => s,
        Err(_) => {
            return Json(Response::auth_error());
        }
    };

    let found_user = match data.user_client.clone().get_user(&body.user_uuid).await {
        Some(user) => user,
        None => {
            return Json(Response::not_found());
        }
    };

    let pub_key = match found_user.pub_key() {
        Ok(key) => key,
        Err(_) => {
            return Json(Response::auth_error());
        }
    };

    if sessionless.verify(message, &pub_key, &sig).is_err() {
        return Json(Response::auth_error());
    }

    let key = PubKeys::key(&body.hash, &pub_key.to_string());
    if data.user_client.clone().delete_user(&found_user.uuid).await {
        if let Err(_) = data.user_client.clone().remove_key(&key).await {
            return Json(Response::server_error("Failed to delete key".to_string()))
        }

        Json(Response::success(202))
    } else {
        Json(Response::server_error("Failed to delete user".to_string()))
    }

}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use sessionless::Sessionless;

    use crate::test_common::{setup_test_server, storage_uri, write_user};


    #[tokio::test]
    async fn test_delete_user_handler() {
        let initial_uuid_1 = "1234";
        let initial_hash_1 = "initial_hash_1";
        let initial_uuid_2 = "1235";
        let initial_hash_2 = "initial_hash_2";

        let timestamp = Utc::now().timestamp().to_string();

        let storage_uri = storage_uri("test_delete_user_handler");
        let test_server = setup_test_server(storage_uri.clone());
        let user_file_path_1 = format!("{}/user:{}", &storage_uri.to_string(), initial_uuid_1);
        let user_file_path_2 = format!("{}/user:{}", &storage_uri.to_string(), initial_uuid_2);
        let key_file_path = format!("{}/keys", &storage_uri.to_string());

        assert!(test_server.is_running());
        let sessionless = Sessionless::new();
        let pub_key = sessionless.public_key();

        // create directory
        assert!(tokio::fs::create_dir_all(&storage_uri.to_string()).await.is_ok());

        // write two users to file
        // and write keys
        assert!(write_user(&user_file_path_1, initial_uuid_1, &pub_key.to_string(), initial_hash_1).await);
        assert!(write_user(&user_file_path_2, initial_uuid_2, &pub_key.to_string(), initial_hash_1).await);
    }
}