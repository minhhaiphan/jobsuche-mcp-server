# Jobsuche MCP Server

An AI-friendly job search integration server using the Model Context Protocol (MCP).
This server provides tools for searching German job listings via the Federal Employment
Agency (Bundesagentur für Arbeit) API.

## Features

- **AI-Friendly Interface**: Simple, semantic parameters for job searching
- **Official API Integration**: Uses the `jobsuche` crate for reliable API access
- **Rich Filtering**: Search by location, job title, employment type, contract type, and more
- **Comprehensive Details**: Get full job information including descriptions and requirements
- **Pagination Support**: Handle large result sets efficiently
- **Zero Configuration**: Works out of the box with sensible defaults

## Installation

### From npm (Recommended)

```bash
npm install -g @wunderfrucht/jobsuche-mcp-server
```

### From Source

```bash
git clone https://github.com/wunderfrucht/jobsuche-mcp-server.git
cd jobsuche-mcp-server
cargo build --release
```

## Configuration

The server uses environment variables for configuration (all optional):

- `JOBSUCHE_API_URL`: API base URL (default: official Bundesagentur für Arbeit API)
- `JOBSUCHE_API_KEY`: Custom API key (default: public API key)
- `JOBSUCHE_DEFAULT_PAGE_SIZE`: Default results per page (default: 25)
- `JOBSUCHE_MAX_PAGE_SIZE`: Maximum results per page (default: 100)

## Usage with MCP Clients

### Claude Desktop

Add to your Claude Desktop configuration (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "jobsuche": {
      "command": "npx",
      "args": ["@wunderfrucht/jobsuche-mcp-server"]
    }
  }
}
```

Or using local binary:

```json
{
  "mcpServers": {
    "jobsuche": {
      "command": "/path/to/jobsuche-mcp-server"
    }
  }
}
```

### Continue.dev

Add to your Continue configuration:

```json
{
  "mcpServers": {
    "jobsuche": {
      "command": "npx",
      "args": ["@wunderfrucht/jobsuche-mcp-server"]
    }
  }
}
```

### n8n (HTTP/SSE Adapter)

n8n’s MCP client expects HTTP or SSE. Use the bundled HTTP/SSE adapter to bridge the
stdio-based MCP server.

1. Install the npm package (global or in your project):

```bash
npm install -g @wunderfrucht/jobsuche-mcp-server
```

2. Start the adapter (spawns the stdio server automatically):

```bash
template-mcp-http-adapter --port 3541
```

3. Configure n8n MCP client to use:

- **HTTP JSON-RPC endpoint:** `http://localhost:3541/rpc`
- **SSE endpoint:** `http://localhost:3541/sse`

#### Adapter configuration

- `--port <number>`: HTTP port (default: 3541)
- `MCP_STDIO_COMMAND`: Override the stdio command to run (defaults to bundled binary)
- `MCP_STDIO_ARGS`: Space-separated args passed to the stdio server
- `MCP_HTTP_TIMEOUT_MS`: RPC timeout in milliseconds (default: 30000)

Example with custom binary and args:

```bash
MCP_STDIO_COMMAND="/path/to/jobsuche-mcp-server" \
MCP_STDIO_ARGS="--log-level debug" \
template-mcp-http-adapter --port 3100
```

### Docker Deployment (HTTP/SSE Adapter)

Build and run the container (adapter listens on 3541 by default):

```bash
docker build -t jobsuche-mcp .
docker run --rm -p 3541:3541 jobsuche-mcp
```

Health check:

```bash
curl -s http://localhost:3541/health
```

JSON-RPC test:

```bash
curl -s http://localhost:3541/rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```

SSE test:

```bash
curl -N http://localhost:3541/sse
```

## Available Tools

### 1. `search_jobs`

Search for jobs in Germany using various filters.

**Note:** For most use cases, consider using `search_jobs_with_details` or `batch_search_jobs` instead, as they are more efficient for AI workflows.

**Parameters:**

- `job_title` (optional): Job title or keywords (e.g., "Software Engineer", "Data Scientist")
- `location` (optional): Location name (e.g., "Berlin", "München", "Deutschland")
- `radius_km` (optional): Search radius in kilometers from the location
- `employment_type` (optional): Employment type filter
  - Options: `"fulltime"`, `"parttime"`, `"mini_job"`, `"home_office"`, `"shift"`
- `contract_type` (optional): Contract type filter
  - Options: `"permanent"`, `"temporary"`
- `published_since_days` (optional): Days since publication (0-100, default: 30)
- `page_size` (optional): Number of results per page (1-100)
- `page` (optional): Page number for pagination (starting from 1)
- `employer` (optional): Employer name to search for (e.g., "BARMER", "Siemens")
- `branch` (optional): Industry/branch to search in (e.g., "IT", "Gesundheitswesen")

**Examples:**

```json
{
  "job_title": "Software Engineer",
  "location": "Berlin",
  "employment_type": ["fulltime"]
}
```

```json
{
  "location": "München",
  "published_since_days": 7,
  "radius_km": 50
}
```

```json
{
  "job_title": "Data Scientist",
  "location": "Deutschland",
  "employment_type": ["fulltime", "parttime"],
  "page_size": 50
}
```

```json
{
  "employer": "BARMER",
  "location": "Wuppertal",
  "employment_type": ["parttime"]
}
```

### 2. `get_job_details`

Get detailed information about a specific job posting.

**Parameters:**

- `reference_number` (required): Job reference number from search results

**Example:**

```json
{
  "reference_number": "10001-1234567890-S"
}
```

### 3. `search_jobs_with_details` ⭐ RECOMMENDED

Search for jobs and automatically fetch full details for top results in a single operation.

**Why use this?** Combines `search_jobs` + multiple `get_job_details` calls into one efficient operation.

**Parameters:**

- All parameters from `search_jobs` (job_title, location, employment_type, etc.)
- `max_details` (optional): Number of jobs to fetch details for (default: 3, max: 10)
- `fields` (optional): Field filtering (see Field Filtering section)

**⚠️ Rate Limiting:** Includes automatic 100ms delays between detail fetches to respect API rate limits.

**Examples:**

```json
{
  "employer": "BARMER",
  "location": "Wuppertal",
  "employment_type": ["parttime"],
  "max_details": 3
}
```

```json
{
  "job_title": "Sekretärin",
  "location": "Wuppertal",
  "radius_km": 25,
  "employment_type": ["parttime"],
  "max_details": 3,
  "fields": {
    "include_fields": ["title", "employer", "salary", "description", "location"]
  }
}
```

**Response includes:**
- Total search results count
- Full details for top N jobs (title, description, salary, requirements, etc.)
- Performance metrics (search_duration_ms, details_duration_ms)

---

### 4. `batch_search_jobs` ⭐⭐ POWER TOOL

Perform multiple different job searches in a single operation - perfect for systematic comparison.

**Why use this?** Compare different employers, job types, or locations simultaneously instead of multiple separate searches.

**Parameters:**

- `searches`: Array of search configurations (max: 5), each with:
  - `name`: Identifier for this search
  - All standard search parameters (job_title, location, employer, etc.)
- `max_details_per_search` (optional): Details to fetch per search (default: 2, max: 5)
- `fields` (optional): Field filtering applied to all results

**⚠️ Rate Limiting:** Includes automatic delays (200ms between searches, 100ms between details) to respect API rate limits. Conservative defaults prevent overwhelming the API.

**Example - Compare Employers:**

```json
{
  "searches": [
    {
      "name": "BARMER Jobs",
      "employer": "BARMER",
      "location": "Wuppertal",
      "employment_type": ["parttime"]
    },
    {
      "name": "Siemens Jobs",
      "employer": "Siemens",
      "location": "Wuppertal",
      "employment_type": ["parttime"]
    }
  ],
  "max_details_per_search": 3
}
```

**Example - Compare Job Types:**

```json
{
  "searches": [
    {
      "name": "Sekretariat",
      "job_title": "Sekretärin",
      "location": "Wuppertal"
    },
    {
      "name": "Sport/Schwimmen",
      "job_title": "Schwimm",
      "location": "Wuppertal"
    },
    {
      "name": "Pädagogik",
      "job_title": "Pädagog",
      "location": "Wuppertal"
    },
    {
      "name": "Verwaltung",
      "job_title": "Verwaltung",
      "branch": "Bildung",
      "location": "Wuppertal"
    }
  ],
  "max_details_per_search": 2,
  "fields": {
    "include_fields": ["title", "employer", "salary", "description"]
  }
}
```

**Response includes:**
- Results for each search (with name for identification)
- Total results found per search
- Full details for top N jobs per search
- Error handling (continues if one search fails)

---

### 5. `get_server_status`

Get server status and connection information.

**Example:**

```json
{}
```

## Response Examples

### Search Jobs Response

```json
{
  "total_results": 1523,
  "current_page": 1,
  "page_size": 25,
  "jobs_count": 25,
  "jobs": [
    {
      "reference_number": "10001-1234567890-S",
      "title": "Software Engineer (m/w/d)",
      "employer": "Example GmbH",
      "location": "Berlin (10115)",
      "published_date": "2025-10-15",
      "external_url": null
    }
  ],
  "search_duration_ms": 342
}
```

### Job Details Response

```json
{
  "reference_number": "10001-1234567890-S",
  "title": "Software Engineer (m/w/d)",
  "description": "We are looking for an experienced software engineer...",
  "employer": "Example GmbH",
  "location": "Berlin",
  "employment_type": "Vollzeit",
  "contract_type": "unbefristet",
  "start_date": "2025-11-01",
  "application_deadline": null,
  "contact_info": null,
  "external_url": null,
  "employer_profile_url": null,
  "partner_url": "https://example.com/partner",
  "salary": "50.000 - 70.000 EUR",
  "contract_duration": "12 Monate",
  "takeover_opportunity": null,
  "job_type": "arbeitsstelle",
  "open_positions": null,
  "company_size": null,
  "employer_description": null,
  "branch": null,
  "published_date": null,
  "first_published": "2025-10-10",
  "only_for_disabled": false,
  "fulltime": true,
  "entry_period": "ab 2025-11-01",
  "publication_period": "2025-10-01 - 2025-11-30",
  "is_minor_employment": false,
  "is_temp_agency": false,
  "is_private_agency": false,
  "career_changer_suitable": true,
  "cipher_number": null,
  "raw_data": { ... }
}
```

**Available Fields:**

- **Basic Information:**
  - `reference_number`: Unique job reference
  - `title`: Job title
  - `description`: Full job description
  - `employer`: Company name
  - `location`: Job location

- **Employment Details:**
  - `employment_type`: Type of employment (Vollzeit, Teilzeit, derived from fulltime flag)
  - `fulltime`: Boolean indicator for full-time employment (new in v0.2.0)
  - `contract_duration`: Duration of contract (if temporary)
  - `start_date`: Expected start date (formatted from entry_period)
  - `entry_period`: Entry date range (new in v0.2.0)
  - `publication_period`: Publication date range (new in v0.2.0)

- **Employment Types (new in v0.2.0):**
  - `is_minor_employment`: Geringfügige Beschäftigung/Minijob
  - `is_temp_agency`: Temporary employment agency (Zeitarbeit)
  - `is_private_agency`: Private employment agency
  - `career_changer_suitable`: Suitable for career changers (Quereinsteiger)

- **Compensation:**
  - `salary`: Salary information (if available)

- **Application Information:**
  - `external_url`: External application URL (may be available in search results)
  - `partner_url`: Partner/alliance URL
  - `cipher_number`: Cipher number for anonymous postings (new in v0.2.0)
  - `application_deadline`: Application deadline (not available in API)
  - `contact_info`: Contact information (not available in API)

- **Additional Information:**
  - `job_type`: Type of position (arbeitsstelle, ausbildung, praktikum)
  - `first_published`: First publication date
  - `only_for_disabled`: Only for severely disabled persons
  - `raw_data`: Complete raw API response

- **Fields No Longer Available (API v0.3.0):**
  - `employer_profile_url`: Removed from API
  - `takeover_opportunity`: Removed from API
  - `open_positions`: Removed from API
  - `company_size`: Removed from API
  - `employer_description`: Removed from API
  - `branch`: Removed from API
  - `published_date`: Removed from API
  - `contract_type`: Removed from API

  *These fields remain in the response structure for backward compatibility but will always return `null`.*

## Performance & Efficiency

### Bulk Operations for AI

The new bulk operations in v0.3.0 dramatically reduce the number of tool calls required for common AI workflows:

**Scenario 1: Find and review top 3 jobs**
```
Traditional approach:
- 1x search_jobs
- 3x get_job_details
= 4 tool calls

With search_jobs_with_details:
- 1x search_jobs_with_details (with auto-delays)
= 1 tool call (75% reduction!)
```

**Scenario 2: Compare 4 different job categories**
```
Traditional approach:
- 4x search_jobs
- 8x get_job_details (2 per search)
= 12 tool calls

With batch_search_jobs:
- 1x batch_search_jobs (with auto-delays)
= 1 tool call (92% reduction!)
```

**Rate Limiting Protection:**
- Automatic 100ms delays between detail fetches
- Automatic 200ms delays between searches
- Conservative defaults (max_details: 3, max_details_per_search: 2)
- Relies on jobsuche library's built-in retry logic with exponential backoff

### When to Use What

- **`search_jobs`**: When you only need to see what's available (titles, employers, locations)
- **`get_job_details`**: When you have a specific job reference number
- **`search_jobs_with_details`** ⭐: When you want to search AND review details (most common AI workflow)
- **`batch_search_jobs`** ⭐⭐: When comparing multiple categories (employers, job types, locations)

### Field Filtering (Optional)

Reduce response size and token usage by specifying which fields to include/exclude:

```json
{
  "fields": {
    "include_fields": ["title", "employer", "salary", "description", "location"]
  }
}
```

Or exclude unnecessary fields:

```json
{
  "fields": {
    "exclude_fields": ["raw_data", "cipher_number", "is_temp_agency"]
  }
}
```

**Note:** Field filtering infrastructure is present but full implementation coming in future release.

## Development

### Prerequisites

- Rust 1.75.0 or later
- Node.js 16+ (for npm distribution)

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Testing with MCP Inspector

```bash
npm install -g @modelcontextprotocol/inspector
npx @modelcontextprotocol/inspector ./target/debug/jobsuche-mcp-server
```

### Testing with Direct JSON-RPC

```bash
# List available tools
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/debug/jobsuche-mcp-server

# Call search_jobs
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_jobs","arguments":{"location":"Berlin","page_size":5}}}' | ./target/debug/jobsuche-mcp-server
```

## API Information

This server uses the official Bundesagentur für Arbeit (German Federal Employment Agency) Jobsuche API:

- **Base URL**: `https://rest.arbeitsagentur.de/jobboerse/jobsuche-service`
- **Documentation**: Available via the `jobsuche` Rust crate
- **Rate Limiting**: Subject to API provider limits

### Known API Limitations

- **Contact Information**: The API does not provide direct contact details (email, phone) or application deadlines
- **External URLs**: May only be available in search results, not in detailed job information
- **Employer Search**: Combined with job title in search query (no dedicated filter)
- **Branch Search**: Combined with job title in search query (no dedicated filter)
- **API v0.3.0 Changes**: Several fields from previous API versions are no longer available (see "Fields No Longer Available" section)
- Results are sorted oldest-to-newest (no custom sorting available)
- Maximum 100 results per page
- Job details may return 404 if jobs expire quickly

### Workarounds for Missing Data

- **For Application URLs**: Use the `external_url` field from search results, or check the `partner_url` in job details
- **For Employer-Specific Search**: Use the `employer` parameter which combines with `job_title` in the search
- **For Removed Fields**: Check the `raw_data` field which contains the complete API response - some data may still be available in undocumented fields

## Troubleshooting

### Server won't start

- Check that the API URL is accessible
- Verify environment variables are set correctly
- Ensure you have internet connectivity

### No results found

- Try broader search criteria
- Check spelling of location names
- Increase `published_since_days` parameter
- Try removing employment type filters

### Connection errors

- Verify internet connectivity
- Check if the Bundesagentur für Arbeit API is accessible
- Try the default API URL without custom configuration

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Built with [PulseEngine MCP Framework](https://github.com/wunderfrucht/pulseengine-mcp)
- Uses the [jobsuche](https://crates.io/crates/jobsuche) Rust crate
- Powered by the [Bundesagentur für Arbeit API](https://www.arbeitsagentur.de)

## Support

- [GitHub Issues](https://github.com/wunderfrucht/jobsuche-mcp-server/issues)
- [MCP Specification](https://modelcontextprotocol.io)
- [Jobsuche Crate Documentation](https://docs.rs/jobsuche)
