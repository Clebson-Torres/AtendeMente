import { spawn, execSync } from "child_process";
import { existsSync } from "fs";
import { mkdir, rm } from "fs/promises";
import { join, dirname } from "path";
import { fileURLToPath } from "url";
import http from "http";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const TEMP_DIR = join(
  process.env.TEMP || "/tmp",
  `atendemente-e2e-${Date.now()}`,
);
const PORT = "3001";

async function waitFor(url, label, timeoutMs = 30000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      await new Promise((resolve, reject) => {
        const req = http.get(url, (res) => {
          res.resume();
          resolve(res.statusCode);
        });
        req.on("error", reject);
        req.setTimeout(3000, () => { req.destroy(); reject(new Error("timeout")); });
      });
      console.log(`  ${label} is ready`);
      return true;
    } catch {
      process.stdout.write(".");
      await new Promise((r) => setTimeout(r, 1000));
    }
  }
  console.error(`\n  ${label} failed to start`);
  return false;
}

async function main() {
  console.log("=== AtendeMente E2E Test Runner ===");
  console.log(`Temp dir: ${TEMP_DIR}`);

  await mkdir(TEMP_DIR, { recursive: true });

  // Build server binary
  console.log("\n[1/4] Building server binary...");
  execSync("cargo build --bin server", {
    cwd: join(ROOT, "src-tauri"),
    stdio: "inherit",
  });

  const binName = process.platform === "win32" ? "server.exe" : "server";
  const serverBin = join(ROOT, "src-tauri", "target", "debug", binName);
  if (!existsSync(serverBin)) {
    console.error("Server binary not found");
    process.exit(1);
  }

  // Start Vite
  console.log("\n[2/4] Starting Vite dev server...");
  const vite = spawn("npx", ["vite", "--port", "1420"], {
    cwd: ROOT,
    stdio: "pipe",
    shell: true,
    env: { ...process.env },
  });
  vite.stdout.on("data", (d) => process.stdout.write(`[vite] ${d}`));
  vite.stderr.on("data", (d) => process.stderr.write(`[vite] ${d}`));

  // Start server
  console.log("\n[3/4] Starting API server...");
  const server = spawn(serverBin, ["--port", PORT], {
    stdio: "pipe",
    env: {
      ...process.env,
      MASTER_PEPPER: "0000000000000000000000000000000000000000000000000000000000000000",
      DATABASE_URL: `sqlite:${join(TEMP_DIR, "app.db")}?mode=rwc`,
      AUTH_DATABASE_URL: `sqlite:${join(TEMP_DIR, "auth.db")}?mode=rwc`,
      SERVER_PORT: PORT,
      STORAGE_DIR: TEMP_DIR,
      // Sem isto, cada rodada deixa um diretorio de dados de teste no
      // config real do usuario (~/.config/atendemente/data/<uuid>).
      DATA_DIR: join(TEMP_DIR, "data"),
      RUST_LOG: "info",
    },
  });
  server.stdout.on("data", (d) => process.stdout.write(`[server] ${d}`));
  server.stderr.on("data", (d) => process.stderr.write(`[server] ${d}`));

  // Wait for both
  const viteOk = await waitFor("http://localhost:1420", "Vite");
  const serverOk = await waitFor(`http://localhost:${PORT}/api/health`, "API");

  if (!viteOk || !serverOk) {
    console.error("Failed to start services");
    vite.kill();
    server.kill();
    await rm(TEMP_DIR, { recursive: true, force: true });
    process.exit(1);
  }

  // Run Playwright
  console.log("\n[4/4] Running Playwright tests...");
  const pw = spawn(
    "npx",
    ["playwright", "test", "--config", "e2e/playwright.config.ts"],
    {
      cwd: ROOT,
      stdio: "inherit",
      shell: true,
    },
  );

  pw.on("close", async (code) => {
    console.log(`\nPlaywright exited with code ${code}`);

    // Cleanup
    vite.kill();
    server.kill();
    await rm(TEMP_DIR, { recursive: true, force: true });

    console.log("=== E2E Tests Complete ===");
    process.exit(code ?? 0);
  });

}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
