import { readFile, stat, writeFile, mkdir } from "node:fs/promises";
import path from "node:path";

const RELEASES = "https://github.com/moiCR/rDM/releases/download/";

function argumentsFor(args) {
  const result = new Map();
  for (let index = 0; index < args.length; index += 1) {
    const key = args[index];
    if (!key.startsWith("--")) {
      throw new Error(`Unexpected argument: ${key}`);
    }
    const value = args[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`Missing value for ${key}`);
    }
    result.set(key.slice(2), value);
    index += 1;
  }
  return result;
}

function required(args, key) {
  const value = args.get(key);
  if (!value) {
    throw new Error(`Missing --${key}`);
  }
  return value;
}

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

async function writeJson(file, value) {
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function validateTag(tag, version) {
  if (tag !== `v${version}`) {
    throw new Error(`Release tag ${tag} does not match app version ${version}.`);
  }
}

async function seed(args) {
  const output = required(args, "out");
  const previous = args.get("previous");
  const notesFile = args.get("notes-file");
  const manifest = previous
    ? await readJson(previous)
    : { version: "0.0.0", notes: "", pub_date: new Date().toISOString(), platforms: {} };
  if (!manifest.platforms || typeof manifest.platforms !== "object") {
    throw new Error("Previous updater manifest has no platform map.");
  }
  if (notesFile) {
    manifest.notes = await readFile(notesFile, "utf8");
  }
  manifest.pub_date = new Date().toISOString();
  await writeJson(output, manifest);
}

async function fragment(args) {
  const platform = required(args, "platform");
  const tag = required(args, "tag");
  const version = required(args, "version");
  const directory = required(args, "dir");
  const output = required(args, "out");
  validateTag(tag, version);

  const artifacts = platform === "windows"
    ? [
        { key: "windows-x86_64", name: `rDM-Setup-v${version}-x64.exe` },
        { key: "windows-x86_64-inno", name: `rDM-Setup-v${version}-x64.exe` },
      ]
    : platform === "linux"
      ? [
          { key: "linux-x86_64", name: `rDM-${version}-x86_64.AppImage` },
          { key: "linux-x86_64-appimage", name: `rDM-${version}-x86_64.AppImage` },
          { key: "linux-x86_64-deb", name: `rdm_${version}_amd64.deb` },
          { key: "linux-x86_64-rpm", name: `rdm-${version}-1.x86_64.rpm` },
        ]
      : null;
  if (!artifacts) {
    throw new Error(`Unsupported platform: ${platform}`);
  }

  const platforms = {};
  for (const artifact of artifacts) {
    const artifactPath = path.join(directory, artifact.name);
    const [{ size }, signatureRaw] = await Promise.all([
      stat(artifactPath),
      readFile(`${artifactPath}.sig`, "utf8"),
    ]);
    const signatureText = signatureRaw.trim();
    if (!signatureText) {
      throw new Error(`Missing signature for ${artifact.name}`);
    }
    const url = `${RELEASES}${encodeURIComponent(tag)}/${encodeURIComponent(artifact.name)}`;
    platforms[artifact.key] = {
      signature: signatureText,
      url,
      size,
    };
  }
  await mkdir(path.dirname(output), { recursive: true });
  await writeFile(output, `${JSON.stringify({ platforms })}\n`, "utf8");
}

function mergePlatformMaps(target, fragment) {
  if (!fragment.platforms || typeof fragment.platforms !== "object") {
    throw new Error("Updater fragment has no platform map.");
  }
  return { ...target, ...fragment.platforms };
}

async function combine(args) {
  const version = required(args, "version");
  const notesFile = required(args, "notes-file");
  const output = required(args, "out");
  let platforms = {};
  for (const name of ["WINDOWS_MANIFEST", "LINUX_MANIFEST"]) {
    const content = process.env[name];
    if (content) {
      platforms = mergePlatformMaps(platforms, JSON.parse(content));
    }
  }
  await writeJson(output, {
    version,
    notes: await readFile(notesFile, "utf8"),
    pub_date: new Date().toISOString(),
    platforms,
  });
}

async function merge(args) {
  const version = required(args, "version");
  const tag = required(args, "tag");
  const manifestPath = required(args, "manifest");
  const fragmentPath = required(args, "fragment");
  const output = required(args, "out");
  const notesFile = args.get("notes-file");
  validateTag(tag, version);
  const manifest = await readJson(manifestPath);
  const fragmentManifest = await readJson(fragmentPath);
  if (!manifest.platforms || typeof manifest.platforms !== "object") {
    throw new Error("Existing updater manifest has no platform map.");
  }
  const releasePrefix = `${RELEASES}${encodeURIComponent(tag)}/`;
  const existingReleasePlatforms = Object.fromEntries(
    Object.entries(manifest.platforms).filter(([, value]) =>
      typeof value?.url === "string" && value.url.startsWith(releasePrefix),
    ),
  );
  manifest.version = version;
  if (notesFile) {
    manifest.notes = await readFile(notesFile, "utf8");
  }
  manifest.pub_date = new Date().toISOString();
  manifest.platforms = mergePlatformMaps(existingReleasePlatforms, fragmentManifest);
  await writeJson(output, manifest);
}

async function main() {
  const [command, ...rest] = process.argv.slice(2);
  const args = argumentsFor(rest);
  if (command === "seed") {
    await seed(args);
  } else if (command === "fragment") {
    await fragment(args);
  } else if (command === "combine") {
    await combine(args);
  } else if (command === "merge") {
    await merge(args);
  } else {
    throw new Error(`Unknown command: ${command ?? ""}`);
  }
}

main().catch((error) => {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
});
