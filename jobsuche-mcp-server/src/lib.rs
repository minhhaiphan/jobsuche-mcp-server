//! Jobsuche MCP Server Library
//!
//! An AI-friendly job search integration server using the Model Context Protocol (MCP).
//! This server provides tools for searching German job listings via the Federal Employment
//! Agency (Bundesagentur für Arbeit) API without requiring knowledge of API internals.

use pulseengine_mcp_macros::{mcp_server, mcp_tools};
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, instrument, warn};

pub mod config;
use config::JobsucheConfig;

// ============================================================================
// API Response Types (matching actual API response format)
// ============================================================================

// Allow unknown fields to avoid deserialization errors when API adds new fields
fn default_vec<T>() -> Vec<T> { Vec::new() }

#[derive(Debug, Clone, Deserialize)]
struct ApiSearchResponse {
    #[serde(default = "default_vec")]
    stellenangebote: Vec<ApiJobListing>,
    #[serde(rename = "maxErgebnisse")]
    max_ergebnisse: Option<u64>,
    page: Option<u64>,
    size: Option<u64>,
    // Ignore any additional fields
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiJobListing {
    #[serde(default)]
    beruf: String,
    titel: Option<String>,
    #[serde(default)]
    refnr: String,
    #[serde(default)]
    arbeitsort: ApiArbeitsort,
    #[serde(default)]
    arbeitgeber: String,
    #[serde(rename = "aktuelleVeroeffentlichungsdatum")]
    aktuelle_veroeffentlichungsdatum: Option<String>,
    #[serde(rename = "externeUrl")]
    externe_url: Option<String>,
    // Ignore any additional fields
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ApiArbeitsort {
    ort: Option<String>,
    plz: Option<String>,
    region: Option<String>,
    land: Option<String>,
    // Ignore any additional fields like koordinaten, strasse, entfernung
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ApiJobDetails {
    titel: Option<String>,
    stellenbeschreibung: Option<String>,
    arbeitgeber: Option<String>,
    arbeitsorte: Option<Vec<ApiJobLocation>>,
    #[serde(rename = "arbeitszeitVollzeit")]
    arbeitszeit_vollzeit: Option<bool>,
    verguetung: Option<String>,
    vertragsdauer: Option<String>,
    #[serde(rename = "stellenangebotsArt")]
    stellenangebots_art: Option<String>,
    #[serde(rename = "ersteVeroeffentlichungsdatum")]
    erste_veroeffentlichungsdatum: Option<String>,
    #[serde(rename = "nurFuerSchwerbehinderte")]
    nur_fuer_schwerbehinderte: Option<bool>,
    eintrittszeitraum: Option<ApiDateRange>,
    veroeffentlichungszeitraum: Option<ApiDateRange>,
    #[serde(rename = "istGeringfuegigeBeschaeftigung")]
    ist_geringfuegige_beschaeftigung: Option<bool>,
    #[serde(rename = "istArbeitnehmerUeberlassung")]
    ist_arbeitnehmer_ueberlassung: Option<bool>,
    #[serde(rename = "istPrivateArbeitsvermittlung")]
    ist_private_arbeitsvermittlung: Option<bool>,
    #[serde(rename = "quereinstiegGeeignet")]
    quereinstieg_geeignet: Option<bool>,
    chiffrenummer: Option<String>,
    #[serde(rename = "externeUrl")]
    externe_url: Option<String>,
    #[serde(rename = "allianzpartnerUrl")]
    allianzpartner_url: Option<String>,
    // Ignore any additional fields
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ApiJobLocation {
    adresse: Option<ApiAddress>,
    // Ignore any additional fields
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ApiAddress {
    ort: Option<String>,
    plz: Option<String>,
    // Ignore any additional fields
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ApiDateRange {
    von: Option<String>,
    bis: Option<String>,
    // Ignore any additional fields
    #[serde(flatten)]
    _extra: std::collections::HashMap<String, serde_json::Value>,
}

// ============================================================================
// MCP Response Types
// ============================================================================

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
    /// Employment type filter: "fulltime", "parttime", "mini_job", "home_office"
    pub employment_type: Option<Vec<String>>,
    /// Contract type filter: "permanent", "temporary"
    pub contract_type: Option<Vec<String>>,
    /// Days since publication (0-100, default: 30)
    pub published_since_days: Option<u64>,
    /// Number of results per page (1-100)
    pub page_size: Option<u64>,
    /// Page number for pagination (starting from 1)
    pub page: Option<u64>,
    /// Employer name to search for
    pub employer: Option<String>,
    /// Branch/industry to search in
    pub branch: Option<String>,
}

/// Result from job search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchJobsResult {
    pub total_results: Option<u64>,
    pub current_page: Option<u64>,
    pub page_size: Option<u64>,
    pub jobs_count: usize,
    pub jobs: Vec<JobSummary>,
    pub search_duration_ms: u64,
}

/// Summary information for a job listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    pub reference_number: String,
    pub title: String,
    pub employer: String,
    pub location: String,
    pub published_date: Option<String>,
    pub external_url: Option<String>,
    pub application_url: String,
}

/// Parameters for getting job details
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetJobDetailsParams {
    /// Job reference number (refnr from search results)
    pub reference_number: String,
}

/// Detailed job information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetJobDetailsResult {
    pub reference_number: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub employer: Option<String>,
    pub location: Option<String>,
    pub employment_type: Option<String>,
    pub salary: Option<String>,
    pub contract_duration: Option<String>,
    pub job_type: Option<String>,
    pub first_published: Option<String>,
    pub only_for_disabled: Option<bool>,
    pub fulltime: Option<bool>,
    pub entry_period: Option<String>,
    pub is_minor_employment: Option<bool>,
    pub is_temp_agency: Option<bool>,
    pub career_changer_suitable: Option<bool>,
    pub external_url: Option<String>,
    pub partner_url: Option<String>,
    pub application_url: String,
}

/// Single search configuration for batch operations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchSearchItem {
    /// Name for this search (for identification in results)
    pub name: String,
    /// Job title or keywords
    pub job_title: Option<String>,
    /// Location name
    pub location: Option<String>,
    /// Search radius in kilometers
    pub radius_km: Option<u64>,
    /// Employment type filter
    pub employment_type: Option<Vec<String>>,
    /// Contract type filter
    pub contract_type: Option<Vec<String>>,
    /// Days since publication
    pub published_since_days: Option<u64>,
    /// Employer name to search for
    pub employer: Option<String>,
    /// Branch/industry to search in
    pub branch: Option<String>,
}

/// Parameters for batch_search_jobs
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchSearchJobsParams {
    /// List of searches to perform (max: 5)
    pub searches: Vec<BatchSearchItem>,
    /// Automatically fetch details for top N results per search (default: 2, max: 5)
    pub max_details_per_search: Option<u64>,
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
    /// Job listings with full details
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

// ============================================================================
// API Client
// ============================================================================

struct JobsucheClient {
    client: Client,
    api_url: String,
    api_key: String,
}

impl JobsucheClient {
    fn new(api_url: &str, api_key: Option<&str>) -> anyhow::Result<Self> {
        let client = Client::builder()
            .use_native_tls()
            .build()?;
        
        Ok(Self {
            client,
            api_url: api_url.to_string(),
            api_key: api_key.unwrap_or("jobboerse-jobsuche").to_string(),
        })
    }

    async fn search(&self, params: &SearchParams) -> anyhow::Result<ApiSearchResponse> {
        let mut url = format!("{}/pc/v4/jobs", self.api_url);
        let mut query_parts = Vec::new();

        if let Some(was) = &params.was {
            query_parts.push(format!("was={}", urlencoding::encode(was)));
        }
        if let Some(wo) = &params.wo {
            query_parts.push(format!("wo={}", urlencoding::encode(wo)));
        }
        if let Some(umkreis) = params.umkreis {
            query_parts.push(format!("umkreis={}", umkreis));
        }
        if let Some(size) = params.size {
            query_parts.push(format!("size={}", size));
        }
        if let Some(page) = params.page {
            query_parts.push(format!("page={}", page));
        }
        if let Some(days) = params.veroeffentlichtseit {
            query_parts.push(format!("veroeffentlichtseit={}", days));
        }
        if let Some(ref arbeitszeit) = params.arbeitszeit {
            for az in arbeitszeit {
                query_parts.push(format!("arbeitszeit={}", az));
            }
        }

        if !query_parts.is_empty() {
            url = format!("{}?{}", url, query_parts.join("&"));
        }

        info!("Fetching URL: {}", url);

        let response = self.client
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("API error: {}", status);
        }

        // Get text first for better error handling
        let text = response.text().await?;
        
        match serde_json::from_str::<ApiSearchResponse>(&text) {
            Ok(result) => Ok(result),
            Err(e) => {
                warn!("Failed to parse API response: {}", e);
                warn!("Response body (first 500 chars): {}", &text[..text.len().min(500)]);
                anyhow::bail!("Failed to parse API response: {}", e)
            }
        }
    }

    async fn job_details(&self, refnr: &str) -> anyhow::Result<ApiJobDetails> {
        let url = format!("{}/pc/v4/jobdetails/{}", self.api_url, urlencoding::encode(refnr));

        let response = self.client
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("API error: {}", response.status());
        }

        let result: ApiJobDetails = response.json().await?;
        Ok(result)
    }
}

struct SearchParams {
    was: Option<String>,
    wo: Option<String>,
    umkreis: Option<u64>,
    size: Option<u64>,
    page: Option<u64>,
    veroeffentlichtseit: Option<u64>,
    arbeitszeit: Option<Vec<String>>,
}

// ============================================================================
// MCP Server
// ============================================================================

/// Jobsuche MCP Server
#[mcp_server(
    name = "Jobsuche MCP Server",
    version = "0.3.1",
    description = "AI-friendly job search integration using the German Federal Employment Agency API",
    auth = "disabled"
)]
#[derive(Clone)]
pub struct JobsucheMcpServer {
    start_time: Instant,
    client: Arc<JobsucheClient>,
    config: Arc<JobsucheConfig>,
}

impl Default for JobsucheMcpServer {
    fn default() -> Self {
        panic!("JobsucheMcpServer cannot be created with default(). Use JobsucheMcpServer::new() instead.")
    }
}

impl JobsucheMcpServer {
    #[instrument]
    pub async fn new() -> anyhow::Result<Self> {
        info!("Initializing Jobsuche MCP Server");

        let config = Arc::new(JobsucheConfig::load()?);
        config.validate()?;

        info!("Configuration loaded: API URL = {}", config.api_url);

        let client = JobsucheClient::new(&config.api_url, config.api_key.as_deref())?;

        info!("Jobsuche MCP Server initialized successfully");

        Ok(Self {
            start_time: Instant::now(),
            client: Arc::new(client),
            config,
        })
    }

    fn get_uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    fn parse_employment_type(emp_type: &str) -> Option<String> {
        match emp_type.to_lowercase().as_str() {
            "fulltime" | "full" | "vollzeit" | "vz" => Some("vz".to_string()),
            "parttime" | "part" | "teilzeit" | "tz" => Some("tz".to_string()),
            "mini" | "minijob" | "mini_job" => Some("minijob".to_string()),
            "home" | "homeoffice" | "home_office" | "ho" => Some("ho".to_string()),
            "shift" | "schicht" | "snw" => Some("snw".to_string()),
            _ => None,
        }
    }
}

#[mcp_tools]
impl JobsucheMcpServer {
    /// Search for jobs in Germany using the Federal Employment Agency database
    #[instrument(skip(self))]
    pub async fn search_jobs(&self, params: SearchJobsParams) -> anyhow::Result<SearchJobsResult> {
        info!("Searching jobs with params: {:?}", params);
        let start = Instant::now();

        // Build search query
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

        let arbeitszeit = params.employment_type.as_ref().map(|types| {
            types.iter()
                .filter_map(|t| Self::parse_employment_type(t))
                .collect()
        });

        let page_size = params
            .page_size
            .unwrap_or(self.config.default_page_size)
            .min(self.config.max_page_size);

        let search_params = SearchParams {
            was: if search_terms.is_empty() { None } else { Some(search_terms.join(" ")) },
            wo: params.location,
            umkreis: params.radius_km,
            size: Some(page_size),
            page: params.page,
            veroeffentlichtseit: params.published_since_days,
            arbeitszeit,
        };

        let response = self.client.search(&search_params).await?;

        let jobs: Vec<JobSummary> = response
            .stellenangebote
            .iter()
            .map(|job| {
                let location = format!(
                    "{}{}",
                    job.arbeitsort.ort.as_deref().unwrap_or(""),
                    job.arbeitsort.plz.as_ref().map(|plz| format!(" ({})", plz)).unwrap_or_default()
                );

                // Generate application_url with fallback hierarchy
                let application_url = job.externe_url
                    .clone()
                    .unwrap_or_else(|| {
                        format!("https://www.arbeitsagentur.de/jobsuche/jobdetail/{}", 
                            &job.refnr)
                    });

                JobSummary {
                    reference_number: job.refnr.clone(),
                    title: job.titel.clone().unwrap_or_else(|| job.beruf.clone()),
                    employer: job.arbeitgeber.clone(),
                    location,
                    published_date: job.aktuelle_veroeffentlichungsdatum.clone(),
                    external_url: job.externe_url.clone(),
                    application_url,
                }
            })
            .collect();

        let duration = start.elapsed();
        info!("Search completed: {} jobs found in {:?}", jobs.len(), duration);

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
    #[instrument(skip(self))]
    pub async fn get_job_details(&self, params: GetJobDetailsParams) -> anyhow::Result<GetJobDetailsResult> {
        info!("Getting job details for: {}", params.reference_number);

        let details = self.client.job_details(&params.reference_number).await?;

        let location_str = details.arbeitsorte.as_ref().and_then(|locs| {
            locs.first().and_then(|loc| {
                loc.adresse.as_ref().and_then(|addr| {
                    addr.ort.clone().map(|ort| {
                        if let Some(ref plz) = addr.plz {
                            format!("{} ({})", ort, plz)
                        } else {
                            ort
                        }
                    })
                })
            })
        });

        let entry_period = details.eintrittszeitraum.as_ref().map(|dr| {
            match (&dr.von, &dr.bis) {
                (Some(von), Some(bis)) => format!("{} - {}", von, bis),
                (Some(von), None) => format!("ab {}", von),
                (None, Some(bis)) => format!("bis {}", bis),
                (None, None) => String::new(),
            }
        });

        // Determine the best application URL with fallback hierarchy:
        // 1. external_url (employer's application page)
        // 2. partner_url (partner job board)
        // 3. Internal Bundesagentur für Arbeit URL (always available)
        let application_url = details.externe_url
            .clone()
            .or_else(|| details.allianzpartner_url.clone())
            .unwrap_or_else(|| {
                format!("https://www.arbeitsagentur.de/jobsuche/jobdetail/{}", 
                    &params.reference_number)
            });

        let result = GetJobDetailsResult {
            reference_number: params.reference_number.clone(),
            title: details.titel,
            description: details.stellenbeschreibung,
            employer: details.arbeitgeber,
            location: location_str,
            employment_type: details.arbeitszeit_vollzeit.map(|vz| if vz { "Vollzeit" } else { "Teilzeit" }.to_string()),
            salary: details.verguetung,
            contract_duration: details.vertragsdauer,
            job_type: details.stellenangebots_art,
            first_published: details.erste_veroeffentlichungsdatum,
            only_for_disabled: details.nur_fuer_schwerbehinderte,
            fulltime: details.arbeitszeit_vollzeit,
            entry_period,
            is_minor_employment: details.ist_geringfuegige_beschaeftigung,
            is_temp_agency: details.ist_arbeitnehmer_ueberlassung,
            career_changer_suitable: details.quereinstieg_geeignet,
            external_url: details.externe_url,
            partner_url: details.allianzpartner_url,
            application_url,
        };

        info!("Job details retrieved successfully");
        Ok(result)
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
    #[instrument(skip(self))]
    pub async fn get_server_status(&self) -> anyhow::Result<JobsucheServerStatus> {
        info!("Getting server status");

        // Test API connectivity
        let search_params = SearchParams {
            was: None,
            wo: Some("Berlin".to_string()),
            umkreis: None,
            size: Some(1),
            page: None,
            veroeffentlichtseit: None,
            arbeitszeit: None,
        };

        let connection_status = match self.client.search(&search_params).await {
            Ok(_) => "Connected".to_string(),
            Err(e) => format!("Connection Error: {}", e),
        };

        Ok(JobsucheServerStatus {
            server_name: "Jobsuche MCP Server".to_string(),
            version: "0.3.1".to_string(),
            uptime_seconds: self.get_uptime_seconds(),
            api_url: self.config.api_url.clone(),
            api_connection_status: connection_status,
            tools_count: 4,
        })
    }
}
