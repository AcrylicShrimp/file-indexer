use aws_config::{meta::region::RegionProviderChain, Region};
use aws_sdk_s3::{
    presigning::PresigningConfig,
    types::{CompletedMultipartUpload, CompletedPart},
};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum S3ServiceError {
    #[error("environment variable `AWS_REGION` is unable to be retrieved: {0:#?}")]
    RetrieveAwsRegion(std::env::VarError),

    #[error("environment variable `AWS_S3_BUCKET_NAME` is unable to be retrieved: {0:#?}")]
    RetrieveAwsS3BucketName(std::env::VarError),

    #[error("failed to create multipart upload: {0:#?}")]
    CreateMultipartUpload(
        aws_sdk_s3::error::SdkError<
            aws_sdk_s3::operation::create_multipart_upload::CreateMultipartUploadError,
        >,
    ),

    #[error("failed to complete multipart upload: {0:#?}")]
    CompleteMultipartUpload(
        aws_sdk_s3::error::SdkError<
            aws_sdk_s3::operation::complete_multipart_upload::CompleteMultipartUploadError,
        >,
    ),

    #[error("failed to abort multipart upload: {0:#?}")]
    AbortMultipartUpload(
        aws_sdk_s3::error::SdkError<
            aws_sdk_s3::operation::abort_multipart_upload::AbortMultipartUploadError,
        >,
    ),

    #[error("missing multipart upload id")]
    MissingMultipartUploadId,

    #[error("failed to create presigned url for upload: {0:#?}")]
    CreatePresignedUrlForUpload(
        aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::upload_part::UploadPartError>,
    ),

    #[error("failed to create presigned url for download: {0:#?}")]
    CreatePresignedUrlForDownload(
        aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::get_object::GetObjectError>,
    ),
}

#[derive(Clone)]
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

    async fn check_multipart_upload_exists(
        &self,
        file_id: Uuid,
        upload_id: &str,
    ) -> Result<bool, S3ServiceError> {
        Ok(self
            .client
            .list_parts()
            .bucket(&self.bucket_name)
            .key(file_id)
            .upload_id(upload_id)
            .max_parts(0)
            .send()
            .await
            .is_ok())
    }

    pub async fn create_multipart_upload(
        &self,
        file_id: Uuid,
        mime_type: impl Into<String>,
    ) -> Result<String, S3ServiceError> {
        let response = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket_name)
            .key(file_id)
            .content_type(mime_type)
            .send()
            .await
            .map_err(S3ServiceError::CreateMultipartUpload)?;

        match response.upload_id() {
            Some(upload_id) => Ok(upload_id.to_owned()),
            None => Err(S3ServiceError::MissingMultipartUploadId),
        }
    }

    pub async fn complete_multipart_upload(
        &self,
        file_id: Uuid,
        upload_id: String,
        parts: &[(u32, String)],
    ) -> Result<Option<()>, S3ServiceError> {
        if !self
            .check_multipart_upload_exists(file_id, &upload_id)
            .await?
        {
            return Ok(None);
        }

        let mut upload = CompletedMultipartUpload::builder();

        for (part_number, e_tag) in parts {
            upload = upload.parts(
                CompletedPart::builder()
                    .part_number(*part_number as i32)
                    .e_tag(e_tag)
                    .build(),
            );
        }

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket_name)
            .key(file_id)
            .upload_id(upload_id)
            .multipart_upload(upload.build())
            .send()
            .await
            .map_err(S3ServiceError::CompleteMultipartUpload)?;

        Ok(Some(()))
    }

    pub async fn abort_multipart_upload(
        &self,
        file_id: Uuid,
        upload_id: String,
    ) -> Result<Option<()>, S3ServiceError> {
        if !self
            .check_multipart_upload_exists(file_id, &upload_id)
            .await?
        {
            return Ok(None);
        }

        self.client
            .abort_multipart_upload()
            .bucket(&self.bucket_name)
            .key(file_id)
            .upload_id(upload_id)
            .send()
            .await
            .map_err(S3ServiceError::AbortMultipartUpload)?;

        Ok(Some(()))
    }

    pub async fn generate_presigned_url_for_upload(
        &self,
        file_id: Uuid,
        upload_id: &str,
        part_number: u32,
        expires_in: Duration,
    ) -> Result<String, S3ServiceError> {
        let request = self
            .client
            .upload_part()
            .bucket(&self.bucket_name)
            .key(file_id)
            .upload_id(upload_id)
            .part_number(part_number as i32)
            .presigned(
                PresigningConfig::builder()
                    .expires_in(expires_in)
                    .build()
                    .unwrap(),
            )
            .await
            .map_err(S3ServiceError::CreatePresignedUrlForUpload)?;

        Ok(request.uri().to_owned())
    }

    pub async fn generate_presigned_url_for_download(
        &self,
        file_id: Uuid,
        expires_in: Duration,
    ) -> Result<Option<String>, S3ServiceError> {
        if !self.check_file_exists(file_id).await? {
            return Ok(None);
        }

        let request = self
            .client
            .get_object()
            .bucket(&self.bucket_name)
            .key(file_id)
            .presigned(
                PresigningConfig::builder()
                    .expires_in(expires_in)
                    .build()
                    .unwrap(),
            )
            .await
            .map_err(S3ServiceError::CreatePresignedUrlForDownload)?;

        Ok(Some(request.uri().to_owned()))
    }
}
