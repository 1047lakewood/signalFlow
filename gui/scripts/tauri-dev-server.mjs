import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const viteBin = path.resolve(__dirname, "../node_modules/vite/bin/vite.js");

const child = spawn(process.execPath, [viteBin, "--host", "127.0.0.1", "--port", "1420", "--strictPort"], {
  cwd: path.resolve(__dirname, ".."),
  stdio: "inherit",
  windowsHide: false,
});

let shuttingDown = false;

function shutdown(exitCode = 0) {
  if (shuttingDown) return;
  shuttingDown = true;

  if (child.exitCode !== null || child.killed) {
    process.exit(exitCode);
    return;
  }

  if (process.platform === "win32") {
    const killer = spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
      stdio: "ignore",
      windowsHide: true,
    });
    killer.on("exit", () => process.exit(exitCode));
    killer.on("error", () => process.exit(exitCode));
    return;
  }

  child.kill("SIGTERM");
  setTimeout(() => {
    if (child.exitCode === null) {
      child.kill("SIGKILL");
    }
    process.exit(exitCode);
  }, 1000).unref();
}

child.on("exit", (code, signal) => {
  if (shuttingDown) {
    process.exit(0);
    return;
  }
  if (signal) {
    process.exit(1);
    return;
  }
  process.exit(code ?? 0);
});

child.on("error", (err) => {
  console.error("Failed to start Vite dev server:", err);
  process.exit(1);
});

for (const sig of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.on(sig, () => shutdown(0));
}

