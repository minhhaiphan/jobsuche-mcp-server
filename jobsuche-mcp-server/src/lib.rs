//! Jobsuche MCP Server Library
//!
//! An AI-friendly job search integration server using the Model Context Protocol (MCP).
//! This server provides tools for searching German job listings via the Federal Employment
//! Agency (Bundesagentur für Arbeit) API without requiring knowledge of API internals.
//!
//! ## Features
//!
//! - **AI-Friendly Interface**: Simple, semantic parameters for job searching
//! - **Official API Integration**: Uses the jobsuche crate for reliable API access
//! - **Rich Filtering**: Search by location, job title, employment type, salary, etc.
//! - **Comprehensive Details**: Get full job information including descriptions and requirements
//! - **Pagination Support**: Handle large result sets efficiently

use jobsuche::{Arbeitszeit, Credentials, JobDetails, JobSearchResponse, JobsucheAsync, SearchOptions};
use pulseengine_mcp_macros::{mcp_server, mcp_tools};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, instrument};

pub mod config;
use config::JobsucheConfig;

/// Server status information
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JobsucheServerStatus {
    pub server_name: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub api_url: String,
    pub api_connection_status: String,
    pub tools_count: usize,
}

/// Parameters for searching jobs
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchJobsParams {
    /// Job title or keywords (e.g., "Software Engineer", "Data Scientist")
    pub job_title: Option<String>,

    /// Location name (e.g., "Berlin", "München", "Deutschland")
    pub location: Option<String>,

    /// Search radius in kilometers from the location (default: 25)
    pub radius_km: Option<u64>,

    /// Employment type filter
    /// Options: "fulltime" (Vollzeit), "parttime" (Teilzeit), "mini_job", "home_office"
    pub employment_type: Option<Vec<String>>,

    /// Contract type filter
    /// Options: "permanent" (unbefristet), "temporary" (befristet)
    pub contract_type: Option<Vec<String>>,

    /// Days since publication (0-100, default: 30)
    /// Example: 7 for jobs posted in the last week
    pub published_since_days: Option<u64>,

    /// Number of results per page (1-100, default from config)
    pub page_size: Option<u64>,

    /// Page number for pagination (starting from 1)
    pub page: Option<u64>,

    /// Employer name to search for
    /// Note: This is combined with job_title in the search query
    /// Example: "BARMER", "Siemens", "Deutsche Bahn"
    pub employer: Option<String>,

    /// Branch/industry to search in
    /// Note: This is combined with job_title in the search query
    /// Example: "IT", "Gesundheitswesen", "Automotive"
    pub branch: Option<String>,
}

/// Result from job search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchJobsResult {
    /// Total number of results found
    pub total_results: Option<u64>,

    /// Current page number
    pub current_page: Option<u64>,

    /// Page size used
    pub page_size: Option<u64>,

    /// Number of jobs in this response
    pub jobs_count: usize,

    /// Job listings
    pub jobs: Vec<JobSummary>,

    /// Search performance info
    pub search_duration_ms: u64,
}

/// Summary information for a job listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    /// Reference number (use this to get job details)
    pub reference_number: String,

    /// Job title
    pub title: String,

    /// Employer name
    pub employer: String,

    /// Location information
    pub location: String,

    /// Publication date (YYYY-MM-DD format)
    pub published_date: Option<String>,

    /// External URL if available
    pub external_url: Option<String>,
}

/// Parameters for getting job details
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetJobDetailsParams {
    /// Job reference number (refnr from search results)
    pub reference_number: String,
}

/// Optional field filtering for responses
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FieldFilter {
    /// Fields to include (if specified, only these fields are returned)
    pub include_fields: Option<Vec<String>>,

    /// Fields to exclude (these fields will be omitted from the response)
    pub exclude_fields: Option<Vec<String>>,
}

/// Parameters for search_jobs_with_details
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchJobsWithDetailsParams {
    /// Search parameters (same as search_jobs)
    pub job_title: Option<String>,
    pub location: Option<String>,
    pub radius_km: Option<u64>,
    pub employment_type: Option<Vec<String>>,
    pub contract_type: Option<Vec<String>>,
    pub published_since_days: Option<u64>,
    pub page_size: Option<u64>,
    pub page: Option<u64>,
    pub employer: Option<String>,
    pub branch: Option<String>,

    /// Automatically fetch details for top N results (default: 3, max: 10)
    pub max_details: Option<u64>,

    /// Optional field filtering to reduce response size
    pub fields: Option<FieldFilter>,
}

/// Result from search_jobs_with_details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchJobsWithDetailsResult {
    /// Total number of results found
    pub total_results: Option<u64>,

    /// Current page number
    pub current_page: Option<u64>,

    /// Page size used
    pub page_size: Option<u64>,

    /// Number of jobs returned
    pub jobs_count: usize,

    /// Job listings with full details
    pub jobs: Vec<GetJobDetailsResult>,

    /// Search performance info
    pub search_duration_ms: u64,

    /// Details fetch performance info
    pub details_duration_ms: u64,
}

/// Single search configuration for batch operations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchSearchItem {
    /// Name for this search (for identification in results)
    pub name: String,

    /// Search parameters
    pub job_title: Option<String>,
    pub location: Option<String>,
    pub radius_km: Option<u64>,
    pub employment_type: Option<Vec<String>>,
    pub contract_type: Option<Vec<String>>,
    pub published_since_days: Option<u64>,
    pub employer: Option<String>,
    pub branch: Option<String>,
}

/// Parameters for batch_search_jobs
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchSearchJobsParams {
    /// List of searches to perform (max: 5)
    pub searches: Vec<BatchSearchItem>,

    /// Automatically fetch details for top N results per search (default: 2, max: 5)
    pub max_details_per_search: Option<u64>,

    /// Optional field filtering to reduce response size
    pub fields: Option<FieldFilter>,
}

/// Result from a single batch search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSearchItemResult {
    /// Name of this search
    pub search_name: String,

    /// Total number of results found
    pub total_results: Option<u64>,

    /// Number of jobs returned with details
    pub jobs_count: usize,

    /// Job listings with full details (if max_details_per_search > 0)
    pub jobs: Vec<GetJobDetailsResult>,

    /// Error message if search failed
    pub error: Option<String>,
}

/// Result from batch_search_jobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSearchJobsResult {
    /// Number of searches performed
    pub searches_count: usize,

    /// Results from each search
    pub results: Vec<BatchSearchItemResult>,

    /// Total execution time
    pub total_duration_ms: u64,
}

/// Detailed job information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetJobDetailsResult {
    /// Reference number
    pub reference_number: String,

    /// Job title
    pub title: Option<String>,

    /// Job description
    pub description: Option<String>,

    /// Employer name
    pub employer: Option<String>,

    /// Location information
    pub location: Option<String>,

    /// Employment type
    pub employment_type: Option<String>,

    /// Contract type
    pub contract_type: Option<String>,

    /// Start date
    pub start_date: Option<String>,

    /// Application deadline (not available in API)
    pub application_deadline: Option<String>,

    /// Contact information (not available in API)
    pub contact_info: Option<String>,

    /// External application URL
    pub external_url: Option<String>,

    /// Employer profile/presentation URL
    pub employer_profile_url: Option<String>,

    /// Partner URL
    pub partner_url: Option<String>,

    /// Salary/compensation information
    pub salary: Option<String>,

    /// Contract duration
    pub contract_duration: Option<String>,

    /// Takeover opportunity after contract (not available in API v0.3.0)
    pub takeover_opportunity: Option<bool>,

    /// Job type (e.g., "arbeitsstelle", "ausbildung", "praktikum")
    pub job_type: Option<String>,

    /// Number of open positions (not available in API v0.3.0)
    pub open_positions: Option<u32>,

    /// Company size (not available in API v0.3.0)
    pub company_size: Option<String>,

    /// Employer description (not available in API v0.3.0)
    pub employer_description: Option<String>,

    /// Industry/branch (not available in API v0.3.0)
    pub branch: Option<String>,

    /// Publication date (not available in API v0.3.0)
    pub published_date: Option<String>,

    /// First publication date
    pub first_published: Option<String>,

    /// Only for severely disabled persons
    pub only_for_disabled: Option<bool>,

    /// Full-time employment
    pub fulltime: Option<bool>,

    /// Entry period (date range)
    pub entry_period: Option<String>,

    /// Publication period (date range)
    pub publication_period: Option<String>,

    /// Minor employment (Geringfügige Beschäftigung/Minijob)
    pub is_minor_employment: Option<bool>,

    /// Temporary employment agency (Zeitarbeit)
    pub is_temp_agency: Option<bool>,

    /// Private employment agency
    pub is_private_agency: Option<bool>,

    /// Suitable for career changers (Quereinsteiger)
    pub career_changer_suitable: Option<bool>,

    /// Cipher number (for anonymous job postings)
    pub cipher_number: Option<String>,

    /// Raw JSON for additional fields
    pub raw_data: serde_json::Value,
}

/// Jobsuche MCP Server
///
/// Main server implementation providing AI-friendly tools for German job search.
#[mcp_server(
    name = "Jobsuche MCP Server",
    version = "0.3.0",
    description = "AI-friendly job search integration using the German Federal Employment Agency API",
    auth = "disabled"
)]
#[derive(Clone)]
pub struct JobsucheMcpServer {
    /// Server start time
    start_time: Instant,

    /// Jobsuche API client
    client: Arc<JobsucheAsync>,

    /// Configuration
    config: Arc<JobsucheConfig>,
}

impl Default for JobsucheMcpServer {
    fn default() -> Self {
        panic!("JobsucheMcpServer cannot be created with default(). Use JobsucheMcpServer::new() instead.")
    }
}

impl JobsucheMcpServer {
    /// Create a new Jobsuche MCP Server
    #[instrument]
    pub async fn new() -> anyhow::Result<Self> {
        info!("Initializing Jobsuche MCP Server");

        let config = Arc::new(JobsucheConfig::load()?);
        config.validate()?;

        info!("Configuration loaded: API URL = {}", config.api_url);

        let credentials = if let Some(ref api_key) = config.api_key {
            info!("Using custom API key");
            Credentials::ApiKey(api_key.clone())
        } else {
            info!("Using default API credentials");
            Credentials::default()
        };

        let client = JobsucheAsync::new(&config.api_url, credentials).await?;

        info!("Jobsuche MCP Server initialized successfully");

        Ok(Self {
            start_time: Instant::now(),
            client: Arc::new(client),
            config,
        })
    }

    /// Get server uptime in seconds
    fn get_uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Convert employment type string to Arbeitszeit enum
    fn parse_employment_type(emp_type: &str) -> Option<Arbeitszeit> {
        match emp_type.to_lowercase().as_str() {
            "fulltime" | "full" | "vollzeit" | "vz" => Some(Arbeitszeit::Vollzeit),
            "parttime" | "part" | "teilzeit" | "tz" => Some(Arbeitszeit::Teilzeit),
            "mini" | "minijob" | "mini_job" => Some(Arbeitszeit::Minijob),
            "home" | "homeoffice" | "home_office" | "ho" => Some(Arbeitszeit::HeimTelearbeit),
            "shift" | "schicht" | "snw" => Some(Arbeitszeit::SchichtNachtarbeitWochenende),
            _ => None,
        }
    }
}

/// MCP tools implementation
#[mcp_tools]
impl JobsucheMcpServer {
    /// Search for jobs in Germany using the Federal Employment Agency database
    ///
    /// This tool allows searching for jobs with various filters including location,
    /// job title, employment type, and more. Results include job summaries with
    /// reference numbers that can be used to get detailed information.
    ///
    /// # Examples
    /// - Search for software jobs in Berlin: `{"job_title": "Software Engineer", "location": "Berlin"}`
    /// - Recent jobs in München: `{"location": "München", "published_since_days": 7}`
    /// - Full-time jobs nationwide: `{"employment_type": ["fulltime"]}`
    #[instrument(skip(self))]
    pub async fn search_jobs(&self, params: SearchJobsParams) -> anyhow::Result<SearchJobsResult> {
        info!("Searching jobs with params: {:?}", params);
        let start = Instant::now();

        let mut search_opts = SearchOptions::builder();

        // Build search query combining job_title, employer, and branch
        let mut search_terms = Vec::new();

        if let Some(ref title) = params.job_title {
            search_terms.push(title.clone());
        }

        if let Some(ref employer) = params.employer {
            search_terms.push(employer.clone());
        }

        if let Some(ref branch) = params.branch {
            search_terms.push(branch.clone());
        }

        if !search_terms.is_empty() {
            let combined_query = search_terms.join(" ");
            search_opts.was(&combined_query);
        }

        // Location
        if let Some(ref location) = params.location {
            search_opts.wo(location);
        }

        // Radius
        if let Some(radius) = params.radius_km {
            search_opts.umkreis(radius);
        }

        // Employment type
        if let Some(ref emp_types) = params.employment_type {
            let arbeitszeit: Vec<Arbeitszeit> = emp_types
                .iter()
                .filter_map(|t| Self::parse_employment_type(t))
                .collect();

            if !arbeitszeit.is_empty() {
                search_opts.arbeitszeit(arbeitszeit);
            }
        }

        // Published since
        if let Some(days) = params.published_since_days {
            search_opts.veroeffentlichtseit(days);
        }

        // Pagination
        let page_size = params
            .page_size
            .unwrap_or(self.config.default_page_size)
            .min(self.config.max_page_size);

        search_opts.size(page_size);

        if let Some(page) = params.page {
            search_opts.page(page);
        }

        let options = search_opts.build();
        let response: JobSearchResponse = self.client.search().list(options).await?;

        let jobs: Vec<JobSummary> = response
            .stellenangebote
            .iter()
            .map(|job| {
                let location = format!(
                    "{}{}",
                    job.arbeitsort.ort.as_deref().unwrap_or(""),
                    job.arbeitsort
                        .plz
                        .as_ref()
                        .map(|plz| format!(" ({})", plz))
                        .unwrap_or_default()
                );

                JobSummary {
                    reference_number: job.refnr.clone(),
                    title: job.titel.clone().unwrap_or_else(|| job.beruf.clone()),
                    employer: job.arbeitgeber.clone(),
                    location,
                    published_date: job.aktuelle_veroeffentlichungsdatum.clone(),
                    external_url: job.externe_url.clone(),
                }
            })
            .collect();

        let duration = start.elapsed();
        info!(
            "Search completed: {} jobs found in {:?}",
            jobs.len(),
            duration
        );

        Ok(SearchJobsResult {
            total_results: response.max_ergebnisse,
            current_page: response.page,
            page_size: response.size,
            jobs_count: jobs.len(),
            jobs,
            search_duration_ms: duration.as_millis() as u64,
        })
    }

    /// Get detailed information about a specific job posting
    ///
    /// Retrieves comprehensive information about a job including the full description,
    /// requirements, application instructions, and contact details.
    ///
    /// # Examples
    /// - Get job details: `{"reference_number": "10001-1234567890-S"}`
    #[instrument(skip(self))]
    pub async fn get_job_details(
        &self,
        params: GetJobDetailsParams,
    ) -> anyhow::Result<GetJobDetailsResult> {
        info!("Getting job details for: {}", params.reference_number);

        let details: JobDetails = self.client.job_details(&params.reference_number).await?;

        // Serialize to JSON for raw_data field
        let raw_data = serde_json::to_value(&details)?;

        // Extract location from JobLocation (v0.3.0 structure)
        let location_str = details.arbeitsorte.first().and_then(|loc| {
            loc.adresse
                .as_ref()
                .and_then(|addr| addr.ort.clone())
                .map(|ort| {
                    if let Some(ref plz) = loc.adresse.as_ref().and_then(|a| a.plz.clone()) {
                        format!("{} ({})", ort, plz)
                    } else {
                        ort
                    }
                })
        });

        // Format date ranges as strings
        let entry_period = details.eintrittszeitraum.as_ref().map(|dr| {
            match (&dr.von, &dr.bis) {
                (Some(von), Some(bis)) => format!("{} - {}", von, bis),
                (Some(von), None) => format!("ab {}", von),
                (None, Some(bis)) => format!("bis {}", bis),
                (None, None) => String::new(),
            }
        });

        let publication_period = details.veroeffentlichungszeitraum.as_ref().map(|dr| {
            match (&dr.von, &dr.bis) {
                (Some(von), Some(bis)) => format!("{} - {}", von, bis),
                (Some(von), None) => format!("ab {}", von),
                (None, Some(bis)) => format!("bis {}", bis),
                (None, None) => String::new(),
            }
        });

        let result = GetJobDetailsResult {
            reference_number: params.reference_number.clone(),
            title: details.titel,
            description: details.stellenbeschreibung,
            employer: details.arbeitgeber,
            location: location_str,
            employment_type: details
                .arbeitszeit_vollzeit
                .map(|vz| if vz { "Vollzeit" } else { "Teilzeit" }.to_string()),
            contract_type: None, // Not available in API v0.3.0
            start_date: entry_period.clone(),
            application_deadline: None, // Not available in API
            contact_info: None,         // Not available in API
            external_url: None,         // Note: May be available in search results, not in details
            employer_profile_url: None, // Not available in API v0.3.0
            partner_url: details.allianzpartner_url,
            salary: details.verguetung,
            contract_duration: details.vertragsdauer,
            takeover_opportunity: None, // Not available in API v0.3.0
            job_type: details.stellenangebots_art,
            open_positions: None,        // Not available in API v0.3.0
            company_size: None,          // Not available in API v0.3.0
            employer_description: None,  // Not available in API v0.3.0
            branch: None,                // Not available in API v0.3.0
            published_date: None,        // Not available in API v0.3.0
            first_published: details.erste_veroeffentlichungsdatum,
            only_for_disabled: details.nur_fuer_schwerbehinderte,
            fulltime: details.arbeitszeit_vollzeit,
            entry_period,
            publication_period,
            is_minor_employment: details.ist_geringfuegige_beschaeftigung,
            is_temp_agency: details.ist_arbeitnehmer_ueberlassung,
            is_private_agency: details.ist_private_arbeitsvermittlung,
            career_changer_suitable: details.quereinstieg_geeignet,
            cipher_number: details.chiffrenummer,
            raw_data,
        };

        info!("Job details retrieved successfully");
        Ok(result)
    }

    /// Search for jobs and automatically fetch details for top results
    ///
    /// This tool combines search_jobs and get_job_details into a single operation,
    /// making it more efficient for AI workflows. It searches for jobs and automatically
    /// fetches full details for the top results.
    ///
    /// # Examples
    /// - Search with auto-details: `{"location": "Wuppertal", "employment_type": ["parttime"], "max_details": 5}`
    /// - With field filtering: `{"employer": "BARMER", "location": "Wuppertal", "max_details": 3, "fields": {"include_fields": ["title", "salary", "description"]}}`
    #[instrument(skip(self))]
    pub async fn search_jobs_with_details(
        &self,
        params: SearchJobsWithDetailsParams,
    ) -> anyhow::Result<SearchJobsWithDetailsResult> {
        info!("Searching jobs with automatic detail fetching");
        let search_start = Instant::now();

        // Convert to SearchJobsParams
        let search_params = SearchJobsParams {
            job_title: params.job_title,
            location: params.location,
            radius_km: params.radius_km,
            employment_type: params.employment_type,
            contract_type: params.contract_type,
            published_since_days: params.published_since_days,
            page_size: params.page_size,
            page: params.page,
            employer: params.employer,
            branch: params.branch,
        };

        // Perform search
        let search_result = self.search_jobs(search_params).await?;
        let search_duration = search_start.elapsed();

        // Determine how many details to fetch (conservative defaults to respect rate limits)
        let max_details = params.max_details.unwrap_or(3).min(10);
        let jobs_to_fetch = search_result
            .jobs
            .iter()
            .take(max_details as usize)
            .collect::<Vec<_>>();

        info!("Fetching details for {} jobs", jobs_to_fetch.len());
        let details_start = Instant::now();

        // Fetch details for each job with delay to respect rate limits
        let mut jobs_with_details = Vec::new();
        for (idx, job) in jobs_to_fetch.iter().enumerate() {
            // Small delay between requests to avoid rate limiting (except first)
            if idx > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }

            match self
                .get_job_details(GetJobDetailsParams {
                    reference_number: job.reference_number.clone(),
                })
                .await
            {
                Ok(details) => jobs_with_details.push(details),
                Err(e) => {
                    info!(
                        "Failed to fetch details for {}: {}",
                        job.reference_number, e
                    );
                    // Continue with other jobs even if one fails
                }
            }
        }

        let details_duration = details_start.elapsed();

        info!(
            "Search completed: {} jobs found, {} details fetched",
            search_result.total_results.unwrap_or(0),
            jobs_with_details.len()
        );

        Ok(SearchJobsWithDetailsResult {
            total_results: search_result.total_results,
            current_page: search_result.current_page,
            page_size: search_result.page_size,
            jobs_count: jobs_with_details.len(),
            jobs: jobs_with_details,
            search_duration_ms: search_duration.as_millis() as u64,
            details_duration_ms: details_duration.as_millis() as u64,
        })
    }

    /// Perform multiple job searches in a single operation
    ///
    /// This tool allows you to search for different types of jobs simultaneously,
    /// making it perfect for comparing opportunities across employers, locations,
    /// or job types. Each search can have different parameters and will return
    /// results independently.
    ///
    /// # Examples
    /// - Compare employers: `{"searches": [{"name": "BARMER", "employer": "BARMER", "location": "Wuppertal"}, {"name": "Siemens", "employer": "Siemens", "location": "Wuppertal"}], "max_details_per_search": 3}`
    /// - Different job types: `{"searches": [{"name": "Sekretariat", "job_title": "Sekretärin"}, {"name": "Sport", "job_title": "Schwimm"}]}`
    #[instrument(skip(self))]
    pub async fn batch_search_jobs(
        &self,
        params: BatchSearchJobsParams,
    ) -> anyhow::Result<BatchSearchJobsResult> {
        let start = Instant::now();
        let searches_count = params.searches.len().min(5); // Limit to 5 searches to respect rate limits

        info!("Performing batch search with {} searches", searches_count);

        let max_details = params.max_details_per_search.unwrap_or(2).min(5);
        let mut results = Vec::new();

        // Process each search
        for (search_idx, search_item) in params.searches.iter().take(searches_count).enumerate() {
            // Small delay between searches to avoid rate limiting (except first)
            if search_idx > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            info!("Processing search: {}", search_item.name);

            // Convert to SearchJobsParams
            let search_params = SearchJobsParams {
                job_title: search_item.job_title.clone(),
                location: search_item.location.clone(),
                radius_km: search_item.radius_km,
                employment_type: search_item.employment_type.clone(),
                contract_type: search_item.contract_type.clone(),
                published_since_days: search_item.published_since_days,
                page_size: Some(max_details),
                page: None,
                employer: search_item.employer.clone(),
                branch: search_item.branch.clone(),
            };

            // Perform search
            let search_result = match self.search_jobs(search_params).await {
                Ok(result) => result,
                Err(e) => {
                    // If search fails, add error result and continue
                    results.push(BatchSearchItemResult {
                        search_name: search_item.name.clone(),
                        total_results: None,
                        jobs_count: 0,
                        jobs: Vec::new(),
                        error: Some(format!("Search failed: {}", e)),
                    });
                    continue;
                }
            };

            // Fetch details if requested (with delay to respect rate limits)
            let mut jobs_with_details = Vec::new();
            if max_details > 0 {
                for (detail_idx, job) in search_result.jobs.iter().take(max_details as usize).enumerate() {
                    // Small delay between detail fetches (except first in this search)
                    if detail_idx > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }

                    match self
                        .get_job_details(GetJobDetailsParams {
                            reference_number: job.reference_number.clone(),
                        })
                        .await
                    {
                        Ok(details) => jobs_with_details.push(details),
                        Err(e) => {
                            info!(
                                "Failed to fetch details for {} in search '{}': {}",
                                job.reference_number, search_item.name, e
                            );
                            // Continue with other jobs even if one fails
                        }
                    }
                }
            }

            results.push(BatchSearchItemResult {
                search_name: search_item.name.clone(),
                total_results: search_result.total_results,
                jobs_count: jobs_with_details.len(),
                jobs: jobs_with_details,
                error: None,
            });
        }

        let duration = start.elapsed();
        info!(
            "Batch search completed: {} searches in {:?}",
            results.len(),
            duration
        );

        Ok(BatchSearchJobsResult {
            searches_count: results.len(),
            results,
            total_duration_ms: duration.as_millis() as u64,
        })
    }

    /// Get server status and connection information
    ///
    /// Returns information about the server status, uptime, API configuration,
    /// and available tools.
    #[instrument(skip(self))]
    pub async fn get_server_status(&self) -> anyhow::Result<JobsucheServerStatus> {
        info!("Getting server status");

        // Test API connectivity by making a minimal search
        let connection_status = match self
            .client
            .search()
            .list(SearchOptions::builder().size(1).build())
            .await
        {
            Ok(_) => "Connected".to_string(),
            Err(e) => format!("Connection Error: {}", e),
        };

        Ok(JobsucheServerStatus {
            server_name: "Jobsuche MCP Server".to_string(),
            version: "0.3.0".to_string(),
            uptime_seconds: self.get_uptime_seconds(),
            api_url: self.config.api_url.clone(),
            api_connection_status: connection_status,
            tools_count: 5, // search_jobs, get_job_details, search_jobs_with_details, batch_search_jobs, get_server_status
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_employment_type_fulltime() {
        assert_eq!(
            JobsucheMcpServer::parse_employment_type("fulltime"),
            Some(Arbeitszeit::Vollzeit)
        );
        assert_eq!(
            JobsucheMcpServer::parse_employment_type("VOLLZEIT"),
            Some(Arbeitszeit::Vollzeit)
        );
        assert_eq!(
            JobsucheMcpServer::parse_employment_type("vz"),
            Some(Arbeitszeit::Vollzeit)
        );
    }

    #[test]
    fn test_parse_employment_type_parttime() {
        assert_eq!(
            JobsucheMcpServer::parse_employment_type("parttime"),
            Some(Arbeitszeit::Teilzeit)
        );
        assert_eq!(
            JobsucheMcpServer::parse_employment_type("teilzeit"),
            Some(Arbeitszeit::Teilzeit)
        );
    }

    #[test]
    fn test_parse_employment_type_minijob() {
        assert_eq!(
            JobsucheMcpServer::parse_employment_type("mini"),
            Some(Arbeitszeit::Minijob)
        );
        assert_eq!(
            JobsucheMcpServer::parse_employment_type("mini_job"),
            Some(Arbeitszeit::Minijob)
        );
    }

    #[test]
    fn test_parse_employment_type_homeoffice() {
        assert_eq!(
            JobsucheMcpServer::parse_employment_type("home"),
            Some(Arbeitszeit::HeimTelearbeit)
        );
        assert_eq!(
            JobsucheMcpServer::parse_employment_type("homeoffice"),
            Some(Arbeitszeit::HeimTelearbeit)
        );
    }

    #[test]
    fn test_parse_employment_type_shift() {
        assert_eq!(
            JobsucheMcpServer::parse_employment_type("shift"),
            Some(Arbeitszeit::SchichtNachtarbeitWochenende)
        );
        assert_eq!(
            JobsucheMcpServer::parse_employment_type("schicht"),
            Some(Arbeitszeit::SchichtNachtarbeitWochenende)
        );
    }

    #[test]
    fn test_parse_employment_type_invalid() {
        assert_eq!(JobsucheMcpServer::parse_employment_type("invalid"), None);
        assert_eq!(JobsucheMcpServer::parse_employment_type(""), None);
    }

    #[test]
    fn test_search_params_serialization() {
        let params = SearchJobsParams {
            job_title: Some("Software Engineer".to_string()),
            location: Some("Berlin".to_string()),
            radius_km: Some(50),
            employment_type: Some(vec!["fulltime".to_string()]),
            contract_type: None,
            published_since_days: Some(7),
            page_size: Some(25),
            page: Some(1),
            employer: None,
            branch: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("Software Engineer"));
        assert!(json.contains("Berlin"));
    }

    #[test]
    fn test_search_params_with_employer() {
        let params = SearchJobsParams {
            job_title: Some("Kundenberaterin".to_string()),
            location: Some("Wuppertal".to_string()),
            radius_km: None,
            employment_type: Some(vec!["parttime".to_string()]),
            contract_type: None,
            published_since_days: None,
            page_size: None,
            page: None,
            employer: Some("BARMER".to_string()),
            branch: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("BARMER"));
        assert!(json.contains("Wuppertal"));
    }

    #[test]
    fn test_search_params_with_branch() {
        let params = SearchJobsParams {
            job_title: None,
            location: Some("München".to_string()),
            radius_km: Some(25),
            employment_type: None,
            contract_type: None,
            published_since_days: Some(14),
            page_size: None,
            page: None,
            employer: None,
            branch: Some("IT".to_string()),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("IT"));
        assert!(json.contains("München"));
    }

    #[test]
    fn test_server_status_serialization() {
        let status = JobsucheServerStatus {
            server_name: "Test Server".to_string(),
            version: "0.3.0".to_string(),
            uptime_seconds: 3600,
            api_url: "https://test.api".to_string(),
            api_connection_status: "Connected".to_string(),
            tools_count: 5,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Test Server"));
        assert!(json.contains("0.3.0"));
        assert!(json.contains("3600"));
    }

    #[test]
    fn test_job_summary_serialization() {
        let summary = JobSummary {
            reference_number: "TEST-123".to_string(),
            title: "Test Job".to_string(),
            employer: "Test Company".to_string(),
            location: "Test City".to_string(),
            published_date: Some("2025-01-01".to_string()),
            external_url: None,
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("TEST-123"));
        assert!(json.contains("Test Job"));
    }

    #[test]
    #[should_panic(expected = "JobsucheMcpServer cannot be created with default()")]
    fn test_default_panics() {
        let _ = JobsucheMcpServer::default();
    }
}

#[test]
fn test_get_job_details_params_serialization() {
    let params = GetJobDetailsParams {
        reference_number: "TEST-REF-123".to_string(),
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("TEST-REF-123"));
}

#[test]
fn test_job_details_result_with_location() {
    let result = GetJobDetailsResult {
        reference_number: "TEST-123".to_string(),
        title: Some("Test Title".to_string()),
        description: Some("Test Description".to_string()),
        employer: Some("Test Employer".to_string()),
        location: Some("Test Location".to_string()),
        employment_type: Some("Vollzeit".to_string()),
        contract_type: None,
        start_date: Some("2025-01-01".to_string()),
        application_deadline: None,
        contact_info: None,
        external_url: None,
        employer_profile_url: None,
        partner_url: None,
        salary: Some("45.000 - 55.000 EUR".to_string()),
        contract_duration: None,
        takeover_opportunity: None,
        job_type: Some("arbeitsstelle".to_string()),
        open_positions: None,
        company_size: None,
        employer_description: None,
        branch: None,
        published_date: None,
        first_published: Some("2025-10-15".to_string()),
        only_for_disabled: Some(false),
        fulltime: Some(true),
        entry_period: Some("ab 2025-01-01".to_string()),
        publication_period: None,
        is_minor_employment: Some(false),
        is_temp_agency: Some(false),
        is_private_agency: Some(false),
        career_changer_suitable: Some(true),
        cipher_number: None,
        raw_data: serde_json::json!({}),
    };

    assert_eq!(result.reference_number, "TEST-123");
    assert_eq!(result.title, Some("Test Title".to_string()));
    assert_eq!(result.location, Some("Test Location".to_string()));
    assert_eq!(result.salary, Some("45.000 - 55.000 EUR".to_string()));
    assert_eq!(result.fulltime, Some(true));
}

#[test]
fn test_search_results_empty() {
    let result = SearchJobsResult {
        total_results: Some(0),
        current_page: Some(1),
        page_size: Some(25),
        jobs_count: 0,
        jobs: vec![],
        search_duration_ms: 100,
    };

    assert_eq!(result.jobs_count, 0);
    assert_eq!(result.jobs.len(), 0);
}

#[test]
fn test_search_results_with_jobs() {
    let jobs = vec![
        JobSummary {
            reference_number: "JOB-1".to_string(),
            title: "Job 1".to_string(),
            employer: "Company 1".to_string(),
            location: "Berlin".to_string(),
            published_date: Some("2025-01-01".to_string()),
            external_url: None,
        },
        JobSummary {
            reference_number: "JOB-2".to_string(),
            title: "Job 2".to_string(),
            employer: "Company 2".to_string(),
            location: "München".to_string(),
            published_date: Some("2025-01-02".to_string()),
            external_url: Some("https://example.com".to_string()),
        },
    ];

    let result = SearchJobsResult {
        total_results: Some(2),
        current_page: Some(1),
        page_size: Some(25),
        jobs_count: 2,
        jobs: jobs.clone(),
        search_duration_ms: 150,
    };

    assert_eq!(result.jobs_count, 2);
    assert_eq!(result.jobs.len(), 2);
    assert_eq!(result.jobs[0].reference_number, "JOB-1");
    assert_eq!(result.jobs[1].title, "Job 2");
    assert_eq!(
        result.jobs[1].external_url,
        Some("https://example.com".to_string())
    );
}

#[test]
fn test_search_jobs_params_defaults() {
    let params = SearchJobsParams {
        job_title: None,
        location: None,
        radius_km: None,
        employment_type: None,
        contract_type: None,
        published_since_days: None,
        page_size: None,
        page: None,
        employer: None,
        branch: None,
    };

    // Test all fields are None
    assert!(params.job_title.is_none());
    assert!(params.location.is_none());
    assert!(params.radius_km.is_none());
    assert!(params.employer.is_none());
    assert!(params.branch.is_none());
}

#[test]
fn test_get_job_details_result_minimal() {
    let result = GetJobDetailsResult {
        reference_number: "MIN-123".to_string(),
        title: None,
        description: None,
        employer: None,
        location: None,
        employment_type: None,
        contract_type: None,
        start_date: None,
        application_deadline: None,
        contact_info: None,
        external_url: None,
        employer_profile_url: None,
        partner_url: None,
        salary: None,
        contract_duration: None,
        takeover_opportunity: None,
        job_type: None,
        open_positions: None,
        company_size: None,
        employer_description: None,
        branch: None,
        published_date: None,
        first_published: None,
        only_for_disabled: None,
        fulltime: None,
        entry_period: None,
        publication_period: None,
        is_minor_employment: None,
        is_temp_agency: None,
        is_private_agency: None,
        career_changer_suitable: None,
        cipher_number: None,
        raw_data: serde_json::json!({"test": "data"}),
    };

    assert_eq!(result.reference_number, "MIN-123");
    assert!(result.title.is_none());
    assert_eq!(result.raw_data["test"], "data");
}

#[test]
fn test_server_status_all_fields() {
    let status = JobsucheServerStatus {
        server_name: "Jobsuche MCP Server".to_string(),
        version: "0.3.0".to_string(),
        uptime_seconds: 12345,
        api_url: "https://rest.arbeitsagentur.de/jobboerse/jobsuche-service".to_string(),
        api_connection_status: "Connected".to_string(),
        tools_count: 5,
    };

    assert_eq!(status.server_name, "Jobsuche MCP Server");
    assert_eq!(status.version, "0.3.0");
    assert_eq!(status.tools_count, 5);
    assert!(status.api_connection_status.contains("Connected"));
}
