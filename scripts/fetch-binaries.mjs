import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const isWindows = process.platform === "win32";
const program = isWindows ? "powershell.exe" : "bash";
const args = isWindows
  ? [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      join(root, "scripts", "fetch-binaries.ps1"),
    ]
  : [join(root, "scripts", "fetch-binaries.sh")];

const result = spawnSync(program, args, {
  cwd: root,
  env: process.env,
  stdio: "inherit",
});

if (result.error) {
  console.error(`Unable to launch ${program}: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
