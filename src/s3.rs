use anyhow::{Context, Result};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::Client;
use bytes::Bytes;

/// Minimum part size for S3 multipart upload (5 MB).
const MIN_PART_SIZE: usize = 5 * 1024 * 1024;

/// S3 multipart uploader that buffers data and uploads in chunks.
pub struct S3MultipartUploader {
    client: Client,
    bucket: String,
    key: String,
    upload_id: String,
    parts: Vec<CompletedPart>,
    buffer: Vec<u8>,
    part_number: i32,
}

impl S3MultipartUploader {
    /// Create a new multipart upload.
    pub async fn new(client: Client, bucket: &str, key: &str) -> Result<Self> {
        let create = client
            .create_multipart_upload()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .context("Failed to create multipart upload")?;

        let upload_id = create
            .upload_id()
            .context("No upload ID returned")?
            .to_string();

        Ok(Self {
            client,
            bucket: bucket.to_string(),
            key: key.to_string(),
            upload_id,
            parts: Vec::new(),
            buffer: Vec::with_capacity(MIN_PART_SIZE * 2),
            part_number: 1,
        })
    }

    /// Write data to the upload buffer.
    /// Automatically flushes parts when buffer exceeds minimum size.
    pub async fn write(&mut self, data: &[u8]) -> Result<()> {
        self.buffer.extend_from_slice(data);

        while self.buffer.len() >= MIN_PART_SIZE {
            self.flush_part(MIN_PART_SIZE).await?;
        }
        Ok(())
    }

    /// Flush a part of the specified size from the buffer.
    async fn flush_part(&mut self, size: usize) -> Result<()> {
        let part_data: Vec<u8> = self.buffer.drain(..size).collect();

        let resp = self
            .client
            .upload_part()
            .bucket(&self.bucket)
            .key(&self.key)
            .upload_id(&self.upload_id)
            .part_number(self.part_number)
            .body(ByteStream::from(Bytes::from(part_data)))
            .send()
            .await
            .with_context(|| format!("Failed to upload part {}", self.part_number))?;

        let e_tag = resp.e_tag().context("No ETag returned for part")?;

        self.parts.push(
            CompletedPart::builder()
                .e_tag(e_tag)
                .part_number(self.part_number)
                .build(),
        );

        self.part_number += 1;
        Ok(())
    }

    /// Complete the multipart upload.
    /// Flushes any remaining buffered data as the final part.
    pub async fn complete(mut self) -> Result<()> {
        // Flush remaining buffer as final part
        if !self.buffer.is_empty() {
            let remaining = self.buffer.len();
            self.flush_part(remaining).await?;
        }

        // S3 requires at least one part
        if self.parts.is_empty() {
            // Upload an empty part if no data was written
            self.client
                .upload_part()
                .bucket(&self.bucket)
                .key(&self.key)
                .upload_id(&self.upload_id)
                .part_number(1)
                .body(ByteStream::from(Bytes::new()))
                .send()
                .await
                .context("Failed to upload empty part")?;
        }

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(&self.key)
            .upload_id(&self.upload_id)
            .multipart_upload(
                CompletedMultipartUpload::builder()
                    .set_parts(Some(self.parts))
                    .build(),
            )
            .send()
            .await
            .context("Failed to complete multipart upload")?;

        Ok(())
    }

    /// Abort the multipart upload.
    /// Call this if an error occurs to clean up incomplete uploads.
    pub async fn abort(self) -> Result<()> {
        self.client
            .abort_multipart_upload()
            .bucket(&self.bucket)
            .key(&self.key)
            .upload_id(&self.upload_id)
            .send()
            .await
            .context("Failed to abort multipart upload")?;

        Ok(())
    }
}
