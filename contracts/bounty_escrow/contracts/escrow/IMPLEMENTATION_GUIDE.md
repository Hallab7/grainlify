# Metadata and Tagging Implementation Guide

## Overview

This guide provides step-by-step instructions for implementing the metadata and tagging functionality tested in `test_metadata_tagging.rs`.

## Current Status

✅ **Completed:**
- Comprehensive test suite created
- Test infrastructure verified
- Documentation written
- Sample payloads defined

❌ **To Implement:**
- EscrowMetadata struct
- Metadata storage and retrieval functions
- Query functions for metadata fields
- Event emissions for metadata changes

## Step 1: Add EscrowMetadata Struct

Add to `lib.rs` after the `Escrow` struct definition:

```rust
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowMetadata {
    pub repo_id: Option<String>,
    pub issue_id: Option<String>,
    pub bounty_type: Option<String>,
    pub tags: Vec<String>,
    pub custom_fields: Vec<(String, String)>,
}
```

## Step 2: Add Storage Key

Add to the `DataKey` enum:

```rust
#[contracttype]
pub enum DataKey {
    // ... existing keys ...
    EscrowMetadata(u64),  // bounty_id -> EscrowMetadata
}
```

## Step 3: Implement Core Metadata Functions

### lock_funds_with_metadata

```rust
pub fn lock_funds_with_metadata(
    env: Env,
    depositor: Address,
    bounty_id: u64,
    amount: i128,
    deadline: u64,
    metadata: EscrowMetadata,
) -> Result<(), Error> {
    // Call existing lock_funds
    Self::lock_funds(env.clone(), depositor, bounty_id, amount, deadline)?;
    
    // Store metadata
    env.storage()
        .persistent()
        .set(&DataKey::EscrowMetadata(bounty_id), &metadata);
    
    // Emit metadata event
    events::emit_metadata_created(&env, bounty_id, metadata);
    
    Ok(())
}
```

### get_escrow_metadata

```rust
pub fn get_escrow_metadata(env: Env, bounty_id: u64) -> EscrowMetadata {
    env.storage()
        .persistent()
        .get(&DataKey::EscrowMetadata(bounty_id))
        .unwrap_or(EscrowMetadata {
            repo_id: None,
            issue_id: None,
            bounty_type: None,
            tags: Vec::new(&env),
            custom_fields: Vec::new(&env),
        })
}
```

### update_escrow_metadata

```rust
pub fn update_escrow_metadata(
    env: Env,
    bounty_id: u64,
    metadata: EscrowMetadata,
) -> Result<(), Error> {
    // Verify escrow exists
    if !env.storage().persistent().has(&DataKey::Escrow(bounty_id)) {
        return Err(Error::BountyNotFound);
    }
    
    // Get escrow to verify depositor
    let escrow: Escrow = env
        .storage()
        .persistent()
        .get(&DataKey::Escrow(bounty_id))
        .unwrap();
    
    // Require depositor authorization
    escrow.depositor.require_auth();
    
    // Update metadata
    env.storage()
        .persistent()
        .set(&DataKey::EscrowMetadata(bounty_id), &metadata);
    
    // Emit update event
    events::emit_metadata_updated(&env, bounty_id, metadata);
    
    Ok(())
}
```

## Step 4: Implement Query Functions

### query_escrows_by_repo_id

```rust
pub fn query_escrows_by_repo_id(
    env: Env,
    repo_id: String,
    offset: u32,
    limit: u32,
) -> Vec<EscrowWithId> {
    let index: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::EscrowIndex)
        .unwrap_or(Vec::new(&env));
    
    let mut results = Vec::new(&env);
    let mut count = 0u32;
    let mut skipped = 0u32;

    for i in 0..index.len() {
        if count >= limit {
            break;
        }

        let bounty_id = index.get(i).unwrap();
        if let Some(metadata) = env
            .storage()
            .persistent()
            .get::<DataKey, EscrowMetadata>(&DataKey::EscrowMetadata(bounty_id))
        {
            if metadata.repo_id == Some(repo_id.clone()) {
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                
                if let Some(escrow) = env
                    .storage()
                    .persistent()
                    .get::<DataKey, Escrow>(&DataKey::Escrow(bounty_id))
                {
                    results.push_back(EscrowWithId { bounty_id, escrow });
                    count += 1;
                }
            }
        }
    }
    results
}
```

### query_escrows_by_bounty_type

```rust
pub fn query_escrows_by_bounty_type(
    env: Env,
    bounty_type: String,
    offset: u32,
    limit: u32,
) -> Vec<EscrowWithId> {
    let index: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::EscrowIndex)
        .unwrap_or(Vec::new(&env));
    
    let mut results = Vec::new(&env);
    let mut count = 0u32;
    let mut skipped = 0u32;

    for i in 0..index.len() {
        if count >= limit {
            break;
        }

        let bounty_id = index.get(i).unwrap();
        if let Some(metadata) = env
            .storage()
            .persistent()
            .get::<DataKey, EscrowMetadata>(&DataKey::EscrowMetadata(bounty_id))
        {
            if metadata.bounty_type == Some(bounty_type.clone()) {
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                
                if let Some(escrow) = env
                    .storage()
                    .persistent()
                    .get::<DataKey, Escrow>(&DataKey::Escrow(bounty_id))
                {
                    results.push_back(EscrowWithId { bounty_id, escrow });
                    count += 1;
                }
            }
        }
    }
    results
}
```

### query_escrows_by_tag

```rust
pub fn query_escrows_by_tag(
    env: Env,
    tag: String,
    offset: u32,
    limit: u32,
) -> Vec<EscrowWithId> {
    let index: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::EscrowIndex)
        .unwrap_or(Vec::new(&env));
    
    let mut results = Vec::new(&env);
    let mut count = 0u32;
    let mut skipped = 0u32;

    for i in 0..index.len() {
        if count >= limit {
            break;
        }

        let bounty_id = index.get(i).unwrap();
        if let Some(metadata) = env
            .storage()
            .persistent()
            .get::<DataKey, EscrowMetadata>(&DataKey::EscrowMetadata(bounty_id))
        {
            // Check if tag exists in tags vector
            let mut has_tag = false;
            for j in 0..metadata.tags.len() {
                if metadata.tags.get(j).unwrap() == tag {
                    has_tag = true;
                    break;
                }
            }
            
            if has_tag {
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                
                if let Some(escrow) = env
                    .storage()
                    .persistent()
                    .get::<DataKey, Escrow>(&DataKey::Escrow(bounty_id))
                {
                    results.push_back(EscrowWithId { bounty_id, escrow });
                    count += 1;
                }
            }
        }
    }
    results
}
```

## Step 5: Add Events

Add to `events.rs`:

```rust
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataCreated {
    pub version: u32,
    pub bounty_id: u64,
    pub repo_id: Option<String>,
    pub issue_id: Option<String>,
    pub bounty_type: Option<String>,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataUpdated {
    pub version: u32,
    pub bounty_id: u64,
    pub timestamp: u64,
}

pub fn emit_metadata_created(env: &Env, bounty_id: u64, metadata: EscrowMetadata) {
    env.events().publish(
        (symbol_short!("metadata"), symbol_short!("created")),
        MetadataCreated {
            version: EVENT_VERSION_V2,
            bounty_id,
            repo_id: metadata.repo_id,
            issue_id: metadata.issue_id,
            bounty_type: metadata.bounty_type,
            timestamp: env.ledger().timestamp(),
        },
    );
}

pub fn emit_metadata_updated(env: &Env, bounty_id: u64, metadata: EscrowMetadata) {
    env.events().publish(
        (symbol_short!("metadata"), symbol_short!("updated")),
        MetadataUpdated {
            version: EVENT_VERSION_V2,
            bounty_id,
            timestamp: env.ledger().timestamp(),
        },
    );
}
```

## Step 6: Run Tests

After implementation, remove `#[ignore]` attributes from tests and run:

```bash
# Run all metadata tests
cargo test test_metadata --lib

# Run specific test
cargo test test_metadata_set_on_creation --lib

# Run with output
cargo test test_metadata --lib -- --nocapture
```

## Step 7: Verify Test Coverage

Ensure all tests pass:

- ✅ test_metadata_set_on_creation
- ✅ test_metadata_update
- ✅ test_metadata_persistence_across_lifecycle
- ✅ test_query_by_repo_id
- ✅ test_query_by_bounty_type
- ✅ test_query_by_tags
- ✅ test_query_filters_on_large_dataset
- ✅ test_metadata_serialization_format
- ✅ test_empty_metadata

## Notes

- All metadata fields are optional to maintain backward compatibility
- Metadata is stored separately from the main Escrow struct
- Query functions use pagination for large datasets
- Events are emitted for indexer compatibility
- Authorization checks ensure only depositors can update metadata

## Similar Implementation for Program Escrow

Follow the same pattern for `program-escrow/src/lib.rs`:

1. Add `ProgramMetadata` struct
2. Add storage key `DataKey::ProgramMetadata(String)`
3. Implement `init_program_with_metadata`, `get_program_metadata`, `update_program_metadata`
4. Implement query functions: `query_programs_by_type`, `query_programs_by_ecosystem`, `query_programs_by_tag`
5. Add events for program metadata changes
6. Run tests in `test_metadata_tagging.rs`
