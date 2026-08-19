/**
 * Drop old IndexFlow tables and re-apply migrations/001_init.sql
 * Usage: node scripts/reset_db.mjs
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pg from "pg";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");

function loadEnv() {
  const envPath = path.join(root, ".env");
  if (!fs.existsSync(envPath)) return;
  for (const line of fs.readFileSync(envPath, "utf8").split(/\r?\n/)) {
    const m = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)\s*$/);
    if (!m) continue;
    let v = m[2];
    if (
      (v.startsWith('"') && v.endsWith('"')) ||
      (v.startsWith("'") && v.endsWith("'"))
    ) {
      v = v.slice(1, -1);
    }
    if (!process.env[m[1]]) process.env[m[1]] = v;
  }
}

loadEnv();

const databaseUrl = process.env.DATABASE_URL;
if (!databaseUrl) {
  console.error("DATABASE_URL is required");
  process.exit(1);
}

const dropSql = `
DROP TABLE IF EXISTS submission_logs CASCADE;
DROP TABLE IF EXISTS health_checks CASCADE;
DROP TABLE IF EXISTS tasks CASCADE;
DROP TABLE IF EXISTS urls CASCADE;
DROP TABLE IF EXISTS sitemaps CASCADE;
DROP TABLE IF EXISTS submit_logs CASCADE;
DROP TABLE IF EXISTS url_queue CASCADE;
DROP TABLE IF EXISTS sites CASCADE;
DROP TABLE IF EXISTS _sqlx_migrations CASCADE;
`;

const migrationPath = path.join(root, "migrations", "001_init.sql");
const migrationSql = fs.readFileSync(migrationPath, "utf8");

const client = new pg.Client({ connectionString: databaseUrl });

try {
  await client.connect();
  console.log("connected");
  await client.query(dropSql);
  console.log("old tables dropped");
  await client.query(migrationSql);
  console.log("schema applied from migrations/001_init.sql");
  // seed sqlx migrations table so app migrate is happy
  await client.query(`
    CREATE TABLE IF NOT EXISTS _sqlx_migrations (
      version BIGINT PRIMARY KEY,
      description TEXT NOT NULL,
      installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
      success BOOLEAN NOT NULL,
      checksum BYTEA NOT NULL,
      execution_time BIGINT NOT NULL
    );
  `);
  console.log("done");
} catch (e) {
  console.error(e);
  process.exit(1);
} finally {
  await client.end();
}
