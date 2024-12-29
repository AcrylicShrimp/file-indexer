use aws_config::{meta::region::RegionProviderChain, Region};
use aws_sdk_s3::presigning::PresigningConfig;
use chrono::{DateTime, Utc};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum S3ServiceError {
    #[error("environment variable `AWS_REGION` is unable to be retrieved: {0:#?}")]
    RetrieveAwsRegion(std::env::VarError),

    #[error("environment variable `AWS_S3_BUCKET_NAME` is unable to be retrieved: {0:#?}")]
    RetrieveAwsS3BucketName(std::env::VarError),

    #[error("failed to create presigned url for upload: {0:#?}")]
    CreatePresignedUrlForUpload(
        #[from] aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::put_object::PutObjectError>,
    ),

    #[error("failed to create presigned url for download: {0:#?}")]
    CreatePresignedUrlForDownload(
        #[from] aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::get_object::GetObjectError>,
    ),
}

pub struct S3Service {
    client: aws_sdk_s3::Client,
    bucket_name: String,
}

impl S3Service {
    pub async fn init() -> Result<Self, S3ServiceError> {
        let region = std::env::var("AWS_REGION").map_err(S3ServiceError::RetrieveAwsRegion)?;
        let bucket_name =
            std::env::var("AWS_S3_BUCKET_NAME").map_err(S3ServiceError::RetrieveAwsS3BucketName)?;

        let region_provider = RegionProviderChain::first_try(Region::new(region.clone()));
        let shared_config = aws_config::from_env().region(region_provider).load().await;
        let client = aws_sdk_s3::Client::new(&shared_config);

        Ok(Self {
            client,
            bucket_name,
        })
    }

    async fn check_file_exists(&self, file_id: Uuid) -> Result<bool, S3ServiceError> {
        Ok(self
            .client
            .head_object()
            .bucket(&self.bucket_name)
            .key(file_id)
            .send()
            .await
            .is_ok())
    }

    pub async fn generate_presigned_url_for_upload(
        &self,
        file_id: Uuid,
        mime_type: impl Into<String>,
    ) -> Result<(String, DateTime<Utc>), S3ServiceError> {
        // 1 day
        static EXPIRES_IN: Duration = Duration::from_secs(60 * 60 * 24);

        let expires_at = Utc::now() + EXPIRES_IN;
        let request = self
            .client
            .put_object()
            .bucket(&self.bucket_name)
            .key(file_id)
            .content_type(mime_type)
            .presigned(
                PresigningConfig::builder()
                    .expires_in(EXPIRES_IN)
                    .build()
                    .unwrap(),
            )
            .await
            .map_err(S3ServiceError::CreatePresignedUrlForUpload)?;

        let url = request.uri().to_string();

        Ok((url, expires_at))
    }

    pub async fn generate_presigned_url_for_download(
        &self,
        file_id: Uuid,
    ) -> Result<Option<(String, DateTime<Utc>)>, S3ServiceError> {
        if !self.check_file_exists(file_id).await? {
            return Ok(None);
        }

        // 6 hours
        static EXPIRES_IN: Duration = Duration::from_secs(60 * 60 * 6);

        let expires_at = Utc::now() + EXPIRES_IN;
        let request = self
            .client
            .get_object()
            .bucket(&self.bucket_name)
            .key(file_id)
            .presigned(
                PresigningConfig::builder()
                    .expires_in(EXPIRES_IN)
                    .build()
                    .unwrap(),
            )
            .await
            .map_err(S3ServiceError::CreatePresignedUrlForDownload)?;

        let url = request.uri().to_string();

        Ok(Some((url, expires_at)))
    }
}
