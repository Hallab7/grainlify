# Sample Metadata Payloads for Testing

This document provides sample metadata payloads for testing escrow metadata and tagging functionality. These payloads can be used by indexers and off-chain systems to validate their parsing logic.

## Bounty Escrow Metadata Samples

### 1. Bug Fix Bounty

```json
{
  "repo_id": "stellar/soroban-examples",
  "issue_id": "GH-123",
  "bounty_type": "bug_fix",
  "tags": ["rust", "critical", "security", "smart-contract"],
  "custom_fields": [
    ["severity", "high"],
    ["estimated_hours", "8"],
    ["requires_review", "true"],
    ["cve_id", "CVE-2024-12345"]
  ]
}
```

**Use Case**: Critical security bug in a smart contract example
**Expected Behavior**: High priority, requires security review

### 2. Feature Request Bounty

```json
{
  "repo_id": "stellar/rs-soroban-sdk",
  "issue_id": "456",
  "bounty_type": "feature",
  "tags": ["enhancement", "beginner-friendly", "documentation"],
  "custom_fields": [
    ["difficulty", "medium"],
    ["category", "api"],
    ["estimated_hours", "16"],
    ["requires_tests", "true"]
  ]
}
```

**Use Case**: New API feature for the Soroban SDK
**Expected Behavior**: Medium difficulty, good for intermediate developers

### 3. Documentation Task

```json
{
  "repo_id": "stellar/soroban-docs",
  "issue_id": "789",
  "bounty_type": "documentation",
  "tags": ["docs", "tutorial", "beginner-friendly", "english"],
  "custom_fields": [
    ["language", "english"],
    ["word_count", "1500"],
    ["target_audience", "beginners"],
    ["format", "markdown"]
  ]
}
```

**Use Case**: Tutorial documentation for beginners
**Expected Behavior**: Accessible to new developers, clear writing required

### 4. Code Review Bounty

```json
{
  "repo_id": "stellar/soroban-cli",
  "issue_id": "PR-234",
  "bounty_type": "code_review",
  "tags": ["review", "rust", "cli", "testing"],
  "custom_fields": [
    ["pr_number", "234"],
    ["lines_changed", "450"],
    ["review_depth", "thorough"],
    ["deadline", "2024-03-15"]
  ]
}
```

**Use Case**: Thorough code review of a CLI pull request
**Expected Behavior**: Detailed review with actionable feedback

### 5. Performance Optimization

```json
{
  "repo_id": "stellar/soroban-rpc",
  "issue_id": "567",
  "bounty_type": "optimization",
  "tags": ["performance", "rust", "advanced", "profiling"],
  "custom_fields": [
    ["current_performance", "100ms"],
    ["target_performance", "50ms"],
    ["profiling_required", "true"],
    ["benchmark_suite", "included"]
  ]
}
```

**Use Case**: Optimize RPC endpoint performance
**Expected Behavior**: Measurable performance improvement with benchmarks

### 6. Multi-Repository Bounty

```json
{
  "repo_id": "stellar/soroban-examples,stellar/rs-soroban-sdk",
  "issue_id": "MULTI-001",
  "bounty_type": "feature",
  "tags": ["cross-repo", "integration", "advanced"],
  "custom_fields": [
    ["affected_repos", "2"],
    ["coordination_required", "true"],
    ["estimated_hours", "40"]
  ]
}
```

**Use Case**: Feature spanning multiple repositories
**Expected Behavior**: Requires coordination across repos

## Program Escrow Metadata Samples

### 1. Hackathon Program

```json
{
  "program_name": "Stellar DeFi Hackathon 2024",
  "program_type": "hackathon",
  "ecosystem": "stellar",
  "tags": ["hackathon", "defi", "smart-contracts", "prizes"],
  "start_date": 1704067200,
  "end_date": 1706745600,
  "custom_fields": [
    ["total_prize_pool", "100000"],
    ["currency", "USDC"],
    ["total_participants", "250"],
    ["sponsor", "Stellar Development Foundation"],
    ["website", "https://stellar.org/hackathon"],
    ["registration_required", "true"]
  ]
}
```

**Use Case**: Large-scale hackathon with multiple prizes
**Expected Behavior**: Batch payouts to multiple winners

### 2. Grant Program

```json
{
  "program_name": "Stellar Ecosystem Grants Q1 2024",
  "program_type": "grant",
  "ecosystem": "stellar",
  "tags": ["grant", "ecosystem", "development", "long-term"],
  "start_date": 1704067200,
  "end_date": 1711929600,
  "custom_fields": [
    ["grant_size_min", "5000"],
    ["grant_size_max", "50000"],
    ["application_required", "true"],
    ["review_period_days", "30"],
    ["focus_areas", "defi,nft,infrastructure"]
  ]
}
```

**Use Case**: Quarterly grant program for ecosystem projects
**Expected Behavior**: Milestone-based payouts

### 3. Bug Bounty Program

```json
{
  "program_name": "Soroban Security Bug Bounty",
  "program_type": "bug_bounty",
  "ecosystem": "stellar",
  "tags": ["security", "bug-bounty", "continuous", "critical"],
  "start_date": 1704067200,
  "end_date": null,
  "custom_fields": [
    ["severity_critical", "10000"],
    ["severity_high", "5000"],
    ["severity_medium", "2000"],
    ["severity_low", "500"],
    ["scope", "soroban-sdk,soroban-cli,soroban-rpc"],
    ["disclosure_policy", "responsible"]
  ]
}
```

**Use Case**: Ongoing security bug bounty program
**Expected Behavior**: Variable payouts based on severity

### 4. Open Source Week Event

```json
{
  "program_name": "Stellar Open Source Week 2024",
  "program_type": "event",
  "ecosystem": "stellar",
  "tags": ["event", "open-source", "community", "beginner-friendly"],
  "start_date": 1709251200,
  "end_date": 1709856000,
  "custom_fields": [
    ["event_type", "virtual"],
    ["total_bounties", "50"],
    ["difficulty_levels", "beginner,intermediate,advanced"],
    ["mentorship_available", "true"],
    ["discord_channel", "stellar-osw-2024"]
  ]
}
```

**Use Case**: Week-long open source contribution event
**Expected Behavior**: Many small payouts to diverse contributors

### 5. Research Program

```json
{
  "program_name": "Stellar Protocol Research Initiative",
  "program_type": "research",
  "ecosystem": "stellar",
  "tags": ["research", "academic", "protocol", "long-term"],
  "start_date": 1704067200,
  "end_date": 1735689600,
  "custom_fields": [
    ["research_areas", "consensus,scalability,privacy"],
    ["publication_required", "true"],
    ["peer_review", "true"],
    ["funding_per_project", "25000"],
    ["collaboration_encouraged", "true"]
  ]
}
```

**Use Case**: Long-term research funding program
**Expected Behavior**: Milestone-based payouts with deliverables

## Query Examples for Indexers

### Query 1: Find All Active Hackathons

```rust
// Query programs by type
let hackathons = contract.query_programs_by_type("hackathon", 0, 50);

// Filter by active (current timestamp between start_date and end_date)
let active_hackathons = hackathons.filter(|p| {
    let metadata = contract.get_program_metadata(&p.program_id);
    let now = env.ledger().timestamp();
    metadata.start_date.unwrap_or(0) <= now && 
    metadata.end_date.unwrap_or(u64::MAX) >= now
});
```

### Query 2: Find Beginner-Friendly Bounties

```rust
// Query by tag
let beginner_bounties = contract.query_escrows_by_tag("beginner-friendly", 0, 100);

// Further filter by bounty type
let beginner_features = beginner_bounties.filter(|e| {
    let metadata = contract.get_escrow_metadata(&e.bounty_id);
    metadata.bounty_type == Some("feature")
});
```

### Query 3: Find High-Value Security Bounties

```rust
// Query by bounty type
let security_bounties = contract.query_escrows_by_bounty_type("bug_fix", 0, 100);

// Filter by security tag and amount
let high_value_security = security_bounties.filter(|e| {
    let metadata = contract.get_escrow_metadata(&e.bounty_id);
    let has_security_tag = metadata.tags.contains("security");
    let is_high_value = e.escrow.amount >= 5000_0000000; // 5000 USDC
    has_security_tag && is_high_value
});
```

### Query 4: Find Bounties by Repository

```rust
// Query by repo_id
let repo_bounties = contract.query_escrows_by_repo_id(
    "stellar/soroban-examples",
    0,
    50
);

// Group by bounty type
let mut by_type = HashMap::new();
for bounty in repo_bounties {
    let metadata = contract.get_escrow_metadata(&bounty.bounty_id);
    let bounty_type = metadata.bounty_type.unwrap_or("unknown");
    by_type.entry(bounty_type).or_insert(Vec::new()).push(bounty);
}
```

### Query 5: Find Programs by Ecosystem

```rust
// Query by ecosystem
let stellar_programs = contract.query_programs_by_ecosystem("stellar", 0, 100);

// Calculate total prize pool
let total_prizes: i128 = stellar_programs.iter()
    .map(|p| contract.get_program_data(&p.program_id).total_funds)
    .sum();
```

## Event Payloads for Indexers

### Metadata Created Event

```json
{
  "event_type": "MetadataCreated",
  "version": 2,
  "bounty_id": 123,
  "repo_id": "stellar/soroban-examples",
  "issue_id": "GH-456",
  "bounty_type": "bug_fix",
  "tags": ["rust", "security"],
  "timestamp": 1704067200
}
```

### Metadata Updated Event

```json
{
  "event_type": "MetadataUpdated",
  "version": 2,
  "bounty_id": 123,
  "updated_fields": ["bounty_type", "tags"],
  "old_bounty_type": "bug_fix",
  "new_bounty_type": "feature",
  "timestamp": 1704153600
}
```

### Program Metadata Created Event

```json
{
  "event_type": "ProgramMetadataCreated",
  "version": 2,
  "program_id": "Hackathon2024",
  "program_name": "Stellar DeFi Hackathon 2024",
  "program_type": "hackathon",
  "ecosystem": "stellar",
  "start_date": 1704067200,
  "end_date": 1706745600,
  "timestamp": 1704067200
}
```

## Validation Rules

### Required Fields
- `repo_id`: Must be in format "org/repo" or "org/repo1,org/repo2"
- `issue_id`: Can be numeric or prefixed (GH-, #, PR-)
- `bounty_type`: Must be one of: bug_fix, feature, documentation, code_review, optimization, research
- `program_type`: Must be one of: hackathon, grant, bug_bounty, event, research

### Optional Fields
- `tags`: Array of strings, max 10 tags
- `custom_fields`: Array of key-value pairs, max 20 fields
- `start_date`: Unix timestamp
- `end_date`: Unix timestamp (must be > start_date if both present)

### String Limits
- `repo_id`: Max 200 characters
- `issue_id`: Max 50 characters
- `bounty_type`: Max 50 characters
- `program_name`: Max 200 characters
- Tag: Max 50 characters each
- Custom field key: Max 50 characters
- Custom field value: Max 500 characters

## Testing Checklist

- [ ] All sample payloads parse correctly
- [ ] Query functions return expected results
- [ ] Events are emitted with correct structure
- [ ] Pagination works for large result sets
- [ ] Special characters are preserved
- [ ] Empty/null fields are handled gracefully
- [ ] Indexers can reconstruct full metadata
- [ ] Performance is acceptable for 1000+ bounties
