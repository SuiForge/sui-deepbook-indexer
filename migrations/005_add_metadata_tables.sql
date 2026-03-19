-- Copyright (c) Mysten Labs, Inc.
-- SPDX-License-Identifier: Apache-2.0

-- Add metadata tables used by the v2 normalized-data scaffolding.

CREATE TABLE IF NOT EXISTS asset_metadata (
    asset_id TEXT PRIMARY KEY,
    coin_type TEXT,
    symbol TEXT,
    name TEXT,
    decimals INT,
    status TEXT,
    source TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS pool_metadata (
    pool_id TEXT PRIMARY KEY,
    base_asset_id TEXT,
    quote_asset_id TEXT,
    package_id TEXT,
    status TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
