#[cfg(test)]
mod tests {
    use std::{env, time::Duration};

    use momento::response::error::MomentoError;
    use momento::{
        response::cache_get_response::MomentoGetStatus, simple_cache_client::SimpleCacheClient,
    };
    use tokio::time::sleep;
    use uuid::Uuid;

    async fn get_momento_instance() -> SimpleCacheClient {
        let auth_token = env::var("TEST_AUTH_TOKEN").expect("env var TEST_AUTH_TOKEN must be set");
        return SimpleCacheClient::new(auth_token, 5).await.unwrap();
    }

    #[tokio::test]
    async fn cache_miss() {
        let cache_name = Uuid::new_v4().to_string();
        let cache_key = Uuid::new_v4().to_string();
        let mut mm = get_momento_instance().await;
        mm.create_cache(&cache_name).await.unwrap();
        let result = mm.get(&cache_name, cache_key).await.unwrap();
        assert!(matches!(result.result, MomentoGetStatus::MISS));
        mm.delete_cache(&cache_name).await.unwrap();
    }

    #[tokio::test]
    async fn cache_validation() {
        let cache_name = "";
        let mut mm = get_momento_instance().await;
        let result = mm.create_cache(cache_name).await.unwrap_err();
        let _err_msg = "Cache name cannot be empty".to_string();
        assert!(matches!(
            result,
            MomentoError::InvalidArgument(_err_message)
        ))
    }

    #[tokio::test]
    async fn ttl_validation() {
        let cache_name = Uuid::new_v4().to_string();
        let cache_key = Uuid::new_v4().to_string();
        let cache_body = Uuid::new_v4().to_string();
        let mut mm = get_momento_instance().await;
        mm.create_cache(&cache_name).await.unwrap();
        let ttl: u32 = 42949678;
        let max_ttl = u32::MAX / 1000 as u32;
        let result = mm
            .set(&cache_name, cache_key, cache_body, Some(ttl.clone())) // 42949678 > 2^32/1000
            .await
            .unwrap_err();
        let _err_message = format!(
            "TTL provided, {}, needs to be less than the maximum TTL {}",
            ttl, max_ttl
        );
        assert!(matches!(
            result,
            MomentoError::InvalidArgument(_err_message)
        ));
        mm.delete_cache(&cache_name).await.unwrap();
    }

    #[tokio::test]
    async fn cache_hit() {
        let cache_name = Uuid::new_v4().to_string();
        let cache_key = Uuid::new_v4().to_string();
        let cache_body = Uuid::new_v4().to_string();
        let mut mm = get_momento_instance().await;
        mm.create_cache(&cache_name).await.unwrap();
        mm.set(&cache_name, cache_key.clone(), cache_body.clone(), None)
            .await
            .unwrap();
        let result = mm.get(&cache_name, cache_key.clone()).await.unwrap();
        assert!(matches!(result.result, MomentoGetStatus::HIT));
        assert_eq!(result.value, cache_body.as_bytes());
        mm.delete_cache(&cache_name).await.unwrap();
    }

    #[tokio::test]
    async fn cache_respects_default_ttl() {
        let cache_name = Uuid::new_v4().to_string();
        let cache_key = Uuid::new_v4().to_string();
        let cache_body = Uuid::new_v4().to_string();
        let mut mm = get_momento_instance().await;
        mm.create_cache(&cache_name).await.unwrap();
        mm.set(&cache_name, cache_key.clone(), cache_body.clone(), None)
            .await
            .unwrap();
        sleep(Duration::new(1, 0)).await;
        let result = mm.get(&cache_name, cache_key.clone()).await.unwrap();
        assert!(matches!(result.result, MomentoGetStatus::HIT));
        mm.delete_cache(&cache_name).await.unwrap();
    }

    #[tokio::test]
    async fn create_cache_then_set() {
        let cache_name = Uuid::new_v4().to_string();
        let cache_key = Uuid::new_v4().to_string();
        let cache_body = Uuid::new_v4().to_string();
        let mut mm = get_momento_instance().await;
        mm.create_cache(&cache_name).await.unwrap();
        mm.set(&cache_name, cache_key.clone(), cache_body.clone(), None)
            .await
            .unwrap();
        let result = mm.get(&cache_name, cache_key.clone()).await.unwrap();
        assert!(matches!(result.result, MomentoGetStatus::HIT));
        assert_eq!(result.value, cache_body.as_bytes());
        mm.delete_cache(&cache_name).await.unwrap();
    }

    #[tokio::test]
    async fn list_caches() {
        let cache_name = Uuid::new_v4().to_string();
        let mut mm = get_momento_instance().await;
        mm.create_cache(&cache_name).await.unwrap();
        mm.list_caches(None).await.unwrap();
        mm.delete_cache(&cache_name).await.unwrap();
    }

    #[tokio::test]
    async fn invalid_control_token_can_still_initialize_sdk() {
        let jwt_header_base64: String = String::from("eyJhbGciOiJIUzUxMiJ9");
        let jwt_invalid_signature_base_64: String =
            String::from("gdghdjjfjyehhdkkkskskmmls76573jnajhjjjhjdhnndy");
        // {"sub":"squirrel","cp":"invalidcontrol.cell-alpha-dev.preprod.a.momentohq.com","c":"cache.cell-alpha-dev.preprod.a.momentohq.com"}
        let jwt_payload_bad_control_plane_base64: String = String::from("eyJzdWIiOiJzcXVpcnJlbCIsImNwIjoiaW52YWxpZGNvbnRyb2wuY2VsbC1hbHBoYS1kZXYucHJlcHJvZC5hLm1vbWVudG9ocS5jb20iLCJjIjoiY2FjaGUuY2VsbC1hbHBoYS1kZXYucHJlcHJvZC5hLm1vbWVudG9ocS5jb20ifQ");
        // This JWT will result in UNAUTHENTICATED from the reachable backend since they have made up signatures
        let bad_control_plane_jwt = jwt_header_base64.clone()
            + "."
            + &jwt_payload_bad_control_plane_base64.clone()
            + "."
            + &jwt_invalid_signature_base_64.clone();
        let mut client = SimpleCacheClient::new(bad_control_plane_jwt, 5)
            .await
            .unwrap();

        // Unable to reach control plane
        let create_cache_result = client.create_cache("cache").await.unwrap_err();
        let _err_msg_internal = "error trying to connect: dns error: failed to lookup address information: nodename nor servname provided, or not known".to_string();
        assert!(matches!(
            create_cache_result,
            MomentoError::InternalServerError(_err_msg_internal)
        ));
        // Can reach data plane
        let set_result = client
            .set("cache", "hello", "world", None)
            .await
            .unwrap_err();
        let _err_msg_unauthenticated = "Invalid signature".to_string();
        assert!(matches!(
            set_result,
            MomentoError::Unauthenticated(_err_msg)
        ));
        let get_result = client.get("cache", "hello").await.unwrap_err();
        assert!(matches!(
            get_result,
            MomentoError::Unauthenticated(_err_msg_unauthenticated)
        ));
    }

    #[tokio::test]
    async fn invalid_data_token_can_still_initialize_sdk() {
        let jwt_header_base64: String = String::from("eyJhbGciOiJIUzUxMiJ9");
        let jwt_invalid_signature_base_64: String =
            String::from("gdghdjjfjyehhdkkkskskmmls76573jnajhjjjhjdhnndy");
        // {"sub":"squirrel","cp":"control.cell-alpha-dev.preprod.a.momentohq.com","c":"invalidcache.cell-alpha-dev.preprod.a.momentohq.com"}
        let jwt_payload_bad_data_plane_base64: String = String::from("eyJzdWIiOiJzcXVpcnJlbCIsImNwIjoiY29udHJvbC5jZWxsLWFscGhhLWRldi5wcmVwcm9kLmEubW9tZW50b2hxLmNvbSIsImMiOiJpbnZhbGlkY2FjaGUuY2VsbC1hbHBoYS1kZXYucHJlcHJvZC5hLm1vbWVudG9ocS5jb20ifQ");
        // This JWT will result in UNAUTHENTICATED from the reachable backend since they have made up signatures
        let bad_data_plane_jwt = jwt_header_base64.clone()
            + "."
            + &jwt_payload_bad_data_plane_base64.clone()
            + "."
            + &jwt_invalid_signature_base_64.clone();
        let mut client = SimpleCacheClient::new(bad_data_plane_jwt, 5).await.unwrap();

        // Can reach control plane
        let create_cache_result = client.create_cache("cache").await.unwrap_err();
        let _err_msg_unauthenticated = "Invalid signature".to_string();
        assert!(matches!(
            create_cache_result,
            MomentoError::Unauthenticated(_err_msg_unauthenticated)
        ));
        // Unable to reach data plane
        let set_result = client
            .set("cache", "hello", "world", None)
            .await
            .unwrap_err();
        let _err_msg_internal = "error trying to connect: dns error: failed to lookup address information: nodename nor servname provided, or not known".to_string();
        assert!(matches!(
            set_result,
            MomentoError::InternalServerError(_err_msg_internal)
        ));
        let get_result = client.get("cache", "hello").await.unwrap_err();
        assert!(matches!(
            get_result,
            MomentoError::InternalServerError(_err_msg_internal)
        ));
    }
}
