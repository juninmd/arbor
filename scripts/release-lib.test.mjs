// node --test scripts/
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseCommit, problems } from "./commits.mjs";
import { bump, bumpFor, latestTag, notes, parseVersion, plan, stamp } from "./release-lib.mjs";

const c = (message, hash = "abcdef0123") => ({ message, hash });

describe("commits semânticos", () => {
  it("lê tipo, escopo, quebra e assunto", () => {
    const p = parseCommit("feat(ui)!: painel de vidro");
    assert.equal(p.type, "feat");
    assert.equal(p.scope, "ui");
    assert.equal(p.breaking, true);
    assert.equal(parseCommit("fix: x\n\nBREAKING CHANGE: y").breaking, true);
    assert.equal(parseCommit("atualizei coisas"), null);
  });

  it("explica o que está errado", () => {
    assert.deepEqual(problems("feat(core): listagem paralela"), []);
    assert.deepEqual(problems("Merge pull request #7 from a/b"), []);
    assert.match(problems("fiz coisas")[0], /not a semantic commit/);
  });
});

describe("planejamento de versão", () => {
  it("feat sobe a minor, o resto sobe o patch", () => {
    assert.equal(bumpFor([c("feat: a"), c("fix: b")], [1, 2, 3]), "minor");
    assert.equal(bumpFor([c("fix: b")], [1, 2, 3]), "patch");
    assert.equal(bumpFor([c("feat!: formato novo")], [0, 4, 1]), "minor");
    assert.equal(bumpFor([c("feat!: formato novo")], [1, 4, 1]), "major");
    assert.deepEqual(bump([1, 4, 1], "patch"), [1, 4, 2]);
  });

  it("planeja a próxima release e ignora o commit da própria release", () => {
    assert.equal(plan([c("chore(release): v1.0.1 [skip ci]")], "v1.0.1", { repo: "o/r" }), null);
    const p = plan([c("feat(ui): aviso de atualização"), c("fix: cursor")], "v0.3.0", { repo: "o/r" });
    assert.equal(p.version, "0.4.0");
    assert.match(p.notes, /## Features\n\n- \*\*ui:\*\* aviso de atualização \(abcdef0\)/);
    assert.match(p.notes, /compare\/v0\.3\.0\.\.\.v0\.4\.0/);
  });

  it("usa a maior tag, não a mais próxima", () => {
    assert.equal(latestTag(["v0.2.0", "v0.10.0", "v0.9.1", "lixo"]), "v0.10.0");
    assert.deepEqual(parseVersion("v1.2.3"), [1, 2, 3]);
    assert.equal(parseVersion("1.2"), null);
  });

  it("lista quebras primeiro", () => {
    const text = notes([c("feat: a"), c("fix!: b")], { version: "1.0.0", previous: null, repo: "o/r" });
    assert.ok(text.indexOf("Breaking changes") < text.indexOf("Features"));
  });
});

describe("carimbo de versão", () => {
  it("troca a versão do workspace e dos crates no lock, e nada mais", () => {
    const toml = '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\nfoo = { version = "0.1.0" }\n';
    const lock =
      '[[package]]\nname = "arbor"\nversion = "0.1.0"\n\n[[package]]\nname = "arbor-core"\nversion = "0.1.0"\n\n[[package]]\nname = "arbor-bench"\nversion = "0.1.0"\n\n[[package]]\nname = "arboard"\nversion = "0.1.0"\n';
    const out = stamp({ "Cargo.toml": toml, "Cargo.lock": lock }, "0.2.0");
    assert.match(out["Cargo.toml"], /\[workspace\.package\]\nversion = "0\.2\.0"/);
    assert.match(out["Cargo.toml"], /foo = \{ version = "0\.1\.0" \}/);
    assert.equal((out["Cargo.lock"].match(/0\.2\.0/g) || []).length, 3);
    assert.match(out["Cargo.lock"], /name = "arboard"\nversion = "0\.1\.0"/);
  });
});
