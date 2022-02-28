use std::convert::TryFrom;
use tonic::{
    codegen::InterceptedService,
    transport::{Channel, ClientTlsConfig, Uri},
    Request,
};

use crate::endpoint_resolver::MomentoEndpointsResolver;
use crate::grpc::cache_header_interceptor::CacheHeaderInterceptor;
use crate::{
    generated::control_client::{
        scs_control_client::ScsControlClient, CreateCacheRequest, DeleteCacheRequest,
        ListCachesRequest,
    },
    grpc::auth_header_interceptor::AuthHeaderInterceptor,
    response::{
        error::MomentoError,
        list_cache_response::{MomentoCache, MomentoListCacheResult},
    },
};

use crate::response::{
    cache_get_response::MomentoGetStatus,
    cache_set_response::{MomentoSetResponse, MomentoSetStatus},
};
use crate::utils;
use crate::{
    generated::cache_client::{scs_client::ScsClient, ECacheResult, GetRequest, SetRequest},
    response::cache_get_response::MomentoGetResponse,
};
pub trait MomentoRequest {
    fn into_bytes(self) -> Vec<u8>;
}

impl MomentoRequest for String {
    fn into_bytes(self) -> Vec<u8> {
        self.into_bytes()
    }
}

impl MomentoRequest for Vec<u8> {
    fn into_bytes(self) -> Vec<u8> {
        self
    }
}

impl MomentoRequest for &str {
    fn into_bytes(self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

#[derive(Clone)]
pub struct SimpleCacheClientBuilder {
    control_channel: Channel,
    data_channel: Channel,
    auth_token: String,
    default_ttl_seconds: u32,
}

impl SimpleCacheClientBuilder {
    pub async fn new(auth_token: String, default_ttl_seconds: u32) -> Result<Self, MomentoError> {
        let momento_endpoints = MomentoEndpointsResolver::resolve(&auth_token, &None);
        let control_endpoint = Uri::try_from(momento_endpoints.control_endpoint.as_str())?;
        let data_endpoint = Uri::try_from(momento_endpoints.data_endpoint.as_str())?;

        let control_channel = Channel::builder(control_endpoint)
            .tls_config(ClientTlsConfig::default())
            .unwrap()
            .connect()
            .await?;

        let data_channel = Channel::builder(data_endpoint)
            .tls_config(ClientTlsConfig::default())
            .unwrap()
            .connect()
            .await?;

        match utils::is_ttl_valid(&default_ttl_seconds) {
            Ok(_) => Ok(Self {
                control_channel,
                data_channel,
                auth_token,
                default_ttl_seconds,
            }),
            Err(e) => Err(e),
        }
    }

    pub fn default_ttl_seconds(mut self, seconds: u32) -> Result<Self, MomentoError> {
        let _ = utils::is_ttl_valid(&seconds)?;
        self.default_ttl_seconds = seconds;
        Ok(self)
    }

    pub fn build(self) -> SimpleCacheClient {
        let control_interceptor = InterceptedService::new(
            self.control_channel.clone(),
            AuthHeaderInterceptor {
                auth_key: self.auth_token.clone(),
            },
        );
        let control_client = ScsControlClient::new(control_interceptor);

        let data_interceptor = InterceptedService::new(
            self.data_channel.clone(),
            CacheHeaderInterceptor {
                auth_key: self.auth_token.clone(),
            },
        );
        let data_client = ScsClient::new(data_interceptor);

        SimpleCacheClient {
            control_client,
            data_client,
            item_default_ttl_seconds: self.default_ttl_seconds,
        }
    }
}

pub struct SimpleCacheClient {
    control_client: ScsControlClient<InterceptedService<Channel, AuthHeaderInterceptor>>,
    data_client: ScsClient<InterceptedService<Channel, CacheHeaderInterceptor>>,
    item_default_ttl_seconds: u32,
}

impl SimpleCacheClient {
    /// Returns an instance of a Momento client
    ///
    /// # Arguments
    ///
    /// * `auth_token` - Momento Token
    /// * `item_default_ttl_seconds` - Default TTL for items put into a cache
    /// # Examples
    ///
    /// ```
    /// # tokio_test::block_on(async {
    ///     use momento::simple_cache_client::SimpleCacheClient;
    ///     use std::env;
    ///     let auth_token = env::var("TEST_AUTH_TOKEN").expect("TEST_AUTH_TOKEN must be set");
    ///     let default_ttl = 30;
    ///     let momento = SimpleCacheClient::new(auth_token, default_ttl).await;
    /// # })
    /// ```
    pub async fn new(auth_token: String, default_ttl_seconds: u32) -> Result<Self, MomentoError> {
        let momento_endpoints = MomentoEndpointsResolver::resolve(&auth_token, &None);
        let control_endpoint = momento_endpoints.control_endpoint.as_str();
        let data_endpoint = momento_endpoints.data_endpoint.as_str();
        let control_client = SimpleCacheClient::build_control_client(
            auth_token.clone(),
            control_endpoint.to_string(),
        )
        .await;
        let data_client =
            SimpleCacheClient::build_data_client(auth_token.clone(), data_endpoint.to_string())
                .await;

        let simple_cache_client = Self {
            control_client: control_client.unwrap(),
            data_client: data_client.unwrap(),
            item_default_ttl_seconds: default_ttl_seconds,
        };
        return Ok(simple_cache_client);
    }

    async fn build_control_client(
        auth_token: String,
        endpoint: String,
    ) -> Result<ScsControlClient<InterceptedService<Channel, AuthHeaderInterceptor>>, MomentoError>
    {
        let uri = Uri::try_from(endpoint)?;
        let channel = Channel::builder(uri)
            .tls_config(ClientTlsConfig::default())
            .unwrap()
            .connect_lazy();

        let interceptor = InterceptedService::new(
            channel.clone(),
            AuthHeaderInterceptor {
                auth_key: auth_token.clone(),
            },
        );
        let client = ScsControlClient::new(interceptor);
        return Ok(client);
    }

    async fn build_data_client(
        auth_token: String,
        endpoint: String,
    ) -> Result<ScsClient<InterceptedService<Channel, CacheHeaderInterceptor>>, MomentoError> {
        let uri = Uri::try_from(endpoint)?;
        let channel = Channel::builder(uri)
            .tls_config(ClientTlsConfig::default())
            .unwrap()
            .connect_lazy();

        let interceptor = InterceptedService::new(
            channel.clone(),
            CacheHeaderInterceptor {
                auth_key: auth_token.clone(),
            },
        );
        let client = ScsClient::new(interceptor);
        return Ok(client);
    }

    /// Creates a new Momento cache
    ///
    /// # Arguments
    ///
    /// * `name` - name of cache to create
    pub async fn create_cache(&mut self, name: &str) -> Result<(), MomentoError> {
        self._is_cache_name_valid(&name)?;
        let request = Request::new(CreateCacheRequest {
            cache_name: name.to_string(),
        });

        self.control_client.create_cache(request).await?;
        Ok(())
    }

    /// Deletes a Momento cache, and all of its contents
    ///
    /// # Arguments
    ///
    /// * `name` - name of cache to delete
    ///
    /// # Examples
    ///
    /// ```
    /// use uuid::Uuid;
    /// # tokio_test::block_on(async {
    ///     use momento::simple_cache_client::SimpleCacheClient;
    ///     use std::env;
    ///     let auth_token = env::var("TEST_AUTH_TOKEN").expect("TEST_AUTH_TOKEN must be set");
    ///     let cache_name = Uuid::new_v4().to_string();
    ///     let mut momento = SimpleCacheClient::new(auth_token, 5).await.unwrap();
    ///     momento.create_cache(&cache_name).await;
    ///     momento.delete_cache(&cache_name).await;
    /// # })
    /// ```
    pub async fn delete_cache(&mut self, name: &str) -> Result<(), MomentoError> {
        self._is_cache_name_valid(&name)?;
        let request = Request::new(DeleteCacheRequest {
            cache_name: name.to_string(),
        });
        self.control_client.delete_cache(request).await?;
        Ok(())
    }

    /// Lists all Momento caches
    ///
    /// # Examples
    ///
    /// ```
    /// use uuid::Uuid;
    /// # tokio_test::block_on(async {
    ///     use momento::simple_cache_client::SimpleCacheClient;
    ///     use std::env;
    ///     let auth_token = env::var("TEST_AUTH_TOKEN").expect("TEST_AUTH_TOKEN must be set");
    ///     let cache_name = Uuid::new_v4().to_string();
    ///     let mut momento = SimpleCacheClient::new(auth_token, 5).await.unwrap();
    ///     momento.create_cache(&cache_name).await;
    ///     let caches = momento.list_caches(None).await;
    ///     momento.delete_cache(&cache_name).await;
    /// # })
    /// ```
    pub async fn list_caches(
        &mut self,
        next_token: Option<&str>,
    ) -> Result<MomentoListCacheResult, MomentoError> {
        let request = Request::new(ListCachesRequest {
            next_token: next_token.unwrap_or_default().to_string(),
        });
        let res = self.control_client.list_caches(request).await?.into_inner();
        let caches = res
            .cache
            .iter()
            .map(|cache| MomentoCache {
                cache_name: cache.cache_name.to_string(),
            })
            .collect();
        let response = MomentoListCacheResult {
            caches,
            next_token: res.next_token.to_string(),
        };
        Ok(response)
    }

    /// Sets an item in a Momento Cache
    ///
    /// # Arguments
    ///
    /// * `cache_name` - name of cache
    /// * `cache_key`
    /// * `cache_body`
    /// * `ttl_seconds` - If None is passed, uses the client's default ttl
    ///
    /// # Examples
    ///
    /// ```
    /// use uuid::Uuid;
    /// # tokio_test::block_on(async {
    ///     use momento::simple_cache_client::SimpleCacheClient;
    ///     use std::env;
    ///     let auth_token = env::var("TEST_AUTH_TOKEN").expect("TEST_AUTH_TOKEN must be set");
    ///     let cache_name = Uuid::new_v4().to_string();
    ///     let mut momento = SimpleCacheClient::new(auth_token, 30).await.unwrap();
    ///     momento.create_cache(&cache_name).await;
    ///     momento.set(&cache_name, "cache_key", "cache_value", None).await;
    ///
    ///     // overriding default ttl
    ///     momento.set(&cache_name, "cache_key", "cache_value", Some(10)).await;
    ///     momento.delete_cache(&cache_name).await;
    /// # })
    /// ```
    pub async fn set<I: MomentoRequest>(
        &mut self,
        cache_name: &str,
        key: I,
        body: I,
        ttl_seconds: Option<u32>,
    ) -> Result<MomentoSetResponse, MomentoError> {
        self._is_cache_name_valid(&cache_name)?;
        let temp_ttl = ttl_seconds.unwrap_or(self.item_default_ttl_seconds);
        let ttl_to_use = match utils::is_ttl_valid(&temp_ttl) {
            Ok(_) => temp_ttl * 1000,
            Err(e) => return Err(e),
        };
        let mut request = tonic::Request::new(SetRequest {
            cache_key: key.into_bytes(),
            cache_body: body.into_bytes(),
            ttl_milliseconds: ttl_to_use,
        });
        request.metadata_mut().append(
            "cache",
            tonic::metadata::AsciiMetadataValue::from_str(&cache_name).unwrap(),
        );
        let _ = self.data_client.set(request).await?;
        Ok(MomentoSetResponse {
            result: MomentoSetStatus::OK,
        })
    }

    /// Gets an item from a Momento Cache
    ///
    /// # Arguments
    ///
    /// * `cache_name` - name of cache
    /// * `key` - cache key
    ///
    /// # Examples
    ///
    /// ```
    /// use uuid::Uuid;
    /// # tokio_test::block_on(async {
    ///     use std::env;
    ///     use momento::{response::cache_get_response::MomentoGetStatus, simple_cache_client::SimpleCacheClient};
    ///     let auth_token = env::var("TEST_AUTH_TOKEN").expect("TEST_AUTH_TOKEN must be set");
    ///     let cache_name = Uuid::new_v4().to_string();
    ///     let mut momento = SimpleCacheClient::new(auth_token, 30).await.unwrap();
    ///     momento.create_cache(&cache_name).await;
    ///     let resp = momento.get(&cache_name, "cache_key").await.unwrap();
    ///     match resp.result {
    ///         MomentoGetStatus::HIT => println!("cache hit!"),
    ///         MomentoGetStatus::MISS => println!("cache miss"),
    ///         _ => println!("error occurred")
    ///     };
    ///
    ///     println!("cache value: {}", resp.as_string());
    ///     momento.delete_cache(&cache_name).await;
    /// # })
    /// ```
    pub async fn get<I: MomentoRequest>(
        &mut self,
        cache_name: &str,
        key: I,
    ) -> Result<MomentoGetResponse, MomentoError> {
        self._is_cache_name_valid(&cache_name)?;
        let mut request = tonic::Request::new(GetRequest {
            cache_key: key.into_bytes(),
        });
        request.metadata_mut().append(
            "cache",
            tonic::metadata::AsciiMetadataValue::from_str(&cache_name).unwrap(),
        );
        let response = self.data_client.get(request).await?.into_inner();
        return match response.result() {
            ECacheResult::Hit => Ok(MomentoGetResponse {
                result: MomentoGetStatus::HIT,
                value: response.cache_body,
            }),
            ECacheResult::Miss => Ok(MomentoGetResponse {
                result: MomentoGetStatus::MISS,
                value: response.cache_body,
            }),
            _ => todo!(),
        };
    }

    fn _is_cache_name_valid(&mut self, cache_name: &str) -> Result<(), MomentoError> {
        if cache_name.trim().is_empty() {
            return Err(MomentoError::InvalidArgument(
                "Cache name cannot be empty".to_string(),
            ));
        }
        return Ok(());
    }
}
