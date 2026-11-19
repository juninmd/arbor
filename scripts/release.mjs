// Usado por .github/workflows/release.yml (mesma abordagem do Vislo).
//   node scripts/release.mjs plan    -> JSON { version, bump, notes, previous } para os commits desde a última tag, ou {}
//   node scripts/release.mjs stamp X -> grava a versão X no Cargo.toml (workspace) e no Cargo.lock
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { latestTag, plan, stamp } from "./release-lib.mjs";

const REPO = process.env.GITHUB_REPOSITORY ?? "juninmd/arbor";
const git = (...args) => execFileSync("git", args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim();

function lastTag() {
  // Todas as tags, não só as alcançáveis: se main andou durante o build, a tag fica no commit construído,
  // fora do histórico de main, e mesmo assim é a última versão publicada. null: nada publicado ainda.
  return latestTag(git("tag", "--list", "v[0-9]*").split(/\s+/));
}

/** Uma release feita depois deste checkout (os runs ficam em fila) pode já conter o HEAD. */
function alreadyReleased() {
  return git("tag", "--contains", "HEAD", "--list", "v[0-9]*") !== "";
}

function commitsSince(tag) {
  const SEP = "\u0001";
  const raw = git("log", "--no-merges", `--format=%H${SEP}%B${SEP}%x00`, tag ? `${tag}..HEAD` : "HEAD");
  return raw
    .split("\0")
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const [hash, message] = entry.split(SEP);
      return { hash, message: message.trim() };
    });
}

function fileVersion() {
  const m = /\[workspace\.package\][^[]*?\nversion\s*=\s*"([^"]+)"/.exec(readFileSync("Cargo.toml", "utf8"));
  return m ? m[1] : "0.0.0";
}

const FILES = ["Cargo.toml", "Cargo.lock"];

const [command, arg] = process.argv.slice(2);
if (command === "plan") {
  const tag = lastTag();
  const result = alreadyReleased() ? null : plan(commitsSince(tag), tag ?? fileVersion(), { repo: REPO });
  console.log(JSON.stringify(result ? { ...result, previous: tag } : {}));
} else if (command === "stamp" && /^\d+\.\d+\.\d+$/.test(arg ?? "")) {
  const files = Object.fromEntries(FILES.map((f) => [f, readFileSync(f, "utf8")]));
  for (const [name, text] of Object.entries(stamp(files, arg))) writeFileSync(name, text);
  console.log(`stamped ${arg}`);
} else {
  console.error("uso: release.mjs plan | stamp <x.y.z>");
  process.exit(2);
}
