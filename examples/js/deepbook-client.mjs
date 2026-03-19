// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

function printHelp() {
  console.log(`DeepBook example client

Usage:
  node examples/js/deepbook-client.mjs [--base-url http://localhost:8080] [--pool-id 0xpool]

Environment:
  DEEPBOOK_BASE_URL   Override API base URL
  DEEPBOOK_POOL_ID    Override pool id used for the summary request
`);
}

function parseArgs(argv) {
  const out = {
    baseURL: process.env.DEEPBOOK_BASE_URL || "http://localhost:8080",
    poolID: process.env.DEEPBOOK_POOL_ID || "0xpool",
  };

  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      out.help = true;
      return out;
    }
    if (arg === "--base-url" && argv[i + 1]) {
      out.baseURL = argv[i + 1];
      i += 1;
      continue;
    }
    if (arg === "--pool-id" && argv[i + 1]) {
      out.poolID = argv[i + 1];
      i += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${arg}`);
  }

  return out;
}

async function fetchJSON(url) {
  const response = await fetch(url);
  if (!response.ok) {
    const body = await response.text();
    throw new Error(`Request failed (${response.status}) for ${url}: ${body}`);
  }
  return response.json();
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const baseURL = args.baseURL.replace(/\/+$/, "");
  const status = await fetchJSON(`${baseURL}/v1/deepbook/status`);
  const topMarkets = await fetchJSON(`${baseURL}/v1/deepbook/markets/top?window=24h&sort=volume_quote&limit=5`);
  const summary = await fetchJSON(`${baseURL}/v1/deepbook/pools/${encodeURIComponent(args.poolID)}/execution/summary?window=24h`);

  console.log(JSON.stringify({
    status,
    top_markets: topMarkets.markets || [],
    summary,
  }, null, 2));
}

main().catch((err) => {
  console.error(err.message || err);
  process.exit(1);
});
