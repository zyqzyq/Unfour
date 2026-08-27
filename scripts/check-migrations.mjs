import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const historicalProMigrationHashes = new Map([
  [
    "20260727120000_pro_workspace_cloud_sync_v1.sql",
    "405abe541d1f6b66cf51c7e69457bdf67561962af137de24f217e0c9c19d6cd6",
  ],
  [
    "20260728100000_pro_cloud_sync_recovery_and_accounts.sql",
    "8401ce8adf1b450e6b1e6d0a4da2c1b03c6b93182ed20d50c834cac7672c3031",
  ],
  [
    "20260729120000_pro_global_sync_settings.sql",
    "2a1c961db9b214f8d770ca00680cc7b524b6205be461830d549fbcf33901c19b",
  ],
  [
    "20260813010000_pro_api_client_cloud_sync_entities.sql",
    "9ada5f6e63f97595a51240ed7c2a468de01d59de9aec4ff01e8bf5ea917289b9",
  ],
  [
    "20260817020000_pro_ssh_task_cloud_sync_entities.sql",
    "2796dd76f442515c9c854fe9114cacbc704bbbc84ee919100dd9fcdbb918f54c",
  ],
  [
    "20260818010000_pro_ssh_task_v3_bootstrap_state.sql",
    "0f197b1e9edd2a09b14340fbc3106c432ac51d110b40681f0ef65db5c29219e5",
  ],
  [
    "20260821055320_pro_connection_cloud_sync_protocol_v4.sql",
    "4932e369653e1e1ec5aef0831665e881883ae4a8ba95b0536ce0fd9322a49345",
  ],
  [
    "20260821120000_pro_connection_cloud_sync_v4_reconciliation_retry.sql",
    "2a27a1b9c9b54e7642a6b79041d7a96e7948d6bfcd8a3696235c81528df92a19",
  ],
]);

const migrationSets = [
  {
    label: "core",
    marker: "_core_",
    required: true,
    dir:
      process.env.UNFOUR_CORE_MIGRATIONS_DIR ??
      path.join(repoRoot, "crates/local-storage/migrations"),
  },
  {
    label: "cloud-sync",
    marker: "_core_",
    legacyMarker: "_pro_",
    expectedHashes: historicalProMigrationHashes,
    required: true,
    dir:
      process.env.UNFOUR_CLOUD_SYNC_MIGRATIONS_DIR ??
      path.join(repoRoot, "crates/unfour-cloud-sync-storage/migrations"),
  },
];

const timestampVersionPattern = /^\d{14}$/;
const seenVersions = new Map();
const errors = [];
const scanned = [];

for (const set of migrationSets) {
  if (!existsSync(set.dir)) {
    if (set.required) {
      errors.push(`${set.label}: migration directory does not exist: ${set.dir}`);
    }
    continue;
  }

  const files = readdirSync(set.dir)
    .filter((file) => file.endsWith(".sql"))
    .sort((a, b) => a.localeCompare(b));

  for (const historicalFile of set.expectedHashes?.keys() ?? []) {
    if (!files.includes(historicalFile)) {
      errors.push(`${set.label}: missing historical migration ${historicalFile}`);
    }
  }

  for (const file of files) {
    const firstUnderscore = file.indexOf("_");
    const version = firstUnderscore > 0 ? file.slice(0, firstUnderscore) : "";
    const entry = `${set.label}:${path.join(set.dir, file)}`;
    scanned.push(entry);

    if (!version || !/^\d+$/.test(version)) {
      errors.push(`${entry}: version must be pure digits before the first "_"`);
      continue;
    }

    if (!timestampVersionPattern.test(version)) {
      errors.push(
        `${entry}: version must be a YYYYMMDDHHMMSS timestamp, not local numbering`,
      );
    }

    const historicalHash = set.expectedHashes?.get(file);
    const requiredMarker = historicalHash ? set.legacyMarker : set.marker;
    if (!file.includes(requiredMarker)) {
      errors.push(`${entry}: filename must include ${requiredMarker}`);
    }

    if (historicalHash) {
      const actualHash = createHash("sha256")
        .update(readFileSync(path.join(set.dir, file)))
        .digest("hex");
      if (actualHash !== historicalHash) {
        errors.push(
          `${entry}: historical checksum changed; expected ${historicalHash}, got ${actualHash}`,
        );
      }
    }

    const previous = seenVersions.get(version);
    if (previous) {
      errors.push(`${entry}: duplicate version ${version}; already used by ${previous}`);
    } else {
      seenVersions.set(version, entry);
    }
  }
}

if (errors.length > 0) {
  console.error("Migration check failed:");
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exit(1);
}

console.log(`Migration check passed (${scanned.length} files).`);
