# file-indexer

A service that manages files by tags.

## Features

- Upload and manage files with tags
- Search and filter files by name, tags, and other metadata
- Automatic re-indexing of files for fast search
- RESTful API interface

## Usage

### Environment Variables

- `AWS_ACCESS_KEY_ID`: The AWS access key ID to use.
- `AWS_SECRET_ACCESS_KEY`: The AWS secret access key to use.
- `AWS_REGION`: The AWS region to use.
- `AWS_S3_BUCKET_NAME`: The AWS S3 bucket name to use.
- `DATABASE_URL`: The URL of the database to use.
- `MEILISEARCH_URL`: The URL of the Meilisearch instance to use.

### Endpoints

#### Files

- `GET /files` - List files with pagination

  - Query Parameters:
    - `limit` (optional, default: 25, range: 1-100) - Number of files to return
    - `last-file-id` (optional) - Last file ID for pagination
    - `last-file-uploaded-at` (optional) - Last file uploaded timestamp for pagination

- `GET /files/<file_id>` - Get file details by ID

- `POST /files/<file_id>/download-urls` - Generate a presigned download URL for a file

- `POST /files` - Create a new file

  - Body: JSON object with file details (name, size, mime_type, tags)

- `POST /files/<file_id>/upload-urls` - Generate a presigned upload URL for a file

- `PATCH /files/<file_id>` - Update file details
  - Body: JSON object with updateable fields (name, size, mime_type, tags)

#### Admin Tasks

- `GET /admin-tasks` - List admin tasks with pagination

  - Query Parameters:
    - `limit` (optional, default: 25, range: 1-100) - Number of tasks to return
    - `last-admin-task-id` (optional) - Last task ID for pagination
    - `last-admin-task-updated-at` (optional) - Last task updated timestamp for pagination

- `GET /admin-tasks/<task_id>` - Get admin task details by ID

- `POST /admin-tasks/re-index` - Trigger a re-indexing task for all files

#### Searches

- `POST /searches/files` - Search files by query and filters
  - Body: JSON object with search parameters (q, limit, filters)

#### About Filters

Filters are nested arrays, outer array is `AND` and inner array is `OR`.

Example: List all files with `pdf`, `jpeg`, or `png` mime type, and a size greater than 5MB.

```json
{
  "q": "pdf or jpeg or png",
  "filters": [
    [
      { "type": "mimeType", "value": "application/pdf" },
      { "type": "mimeType", "value": "image/jpeg" },
      { "type": "mimeType", "value": "image/png" }
    ],
    [{ "type": "size", "operator": "gt", "value": 5000000 }]
  ]
}
```
