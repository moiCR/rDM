import { translations } from "./src/i18n";
import { copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const rootDir = import.meta.dir;
const desktopAssetsDir = join(rootDir, "..", "crates", "app", "public");
const targets = ["chrome", "firefox"];

async function runBuild() {
  for (const target of targets) {
    const outDir = join(rootDir, target);
    for (const [language, messages] of Object.entries(translations)) {
      const localeDir = join(outDir, "_locales", language);
      mkdirSync(localeDir, { recursive: true });
      writeFileSync(join(localeDir, "messages.json"), JSON.stringify({
        extensionName: { message: messages.extensionName },
        extensionDescription: { message: messages.extensionDescription },
      }, null, 2) + "\n");
    }
    const popupOutDir = join(outDir, "popup");
    mkdirSync(popupOutDir, { recursive: true });

    const popupBuild = await Bun.build({
      entrypoints: [join(rootDir, "src", "popup", "main.tsx")],
      outdir: popupOutDir,
      naming: "popup.js",
      target: "browser",
      minify: true,
    });

    if (!popupBuild.success) {
      console.error(`Failed building popup for ${target}:`, popupBuild.logs);
      process.exit(1);
    }

    const bgBuild = await Bun.build({
      entrypoints: [join(rootDir, "src", "background.ts")],
      outdir: outDir,
      naming: "background.js",
      target: "browser",
      minify: false,
    });

    if (!bgBuild.success) {
      console.error(`Failed building background for ${target}:`, bgBuild.logs);
      process.exit(1);
    }

    copyFileSync(
      join(rootDir, "src", "popup", "popup.html"),
      join(popupOutDir, "popup.html")
    );
    copyFileSync(
      join(rootDir, "src", "popup", "popup.css"),
      join(popupOutDir, "popup.css")
    );
    copyFileSync(
      join(rootDir, "THIRD_PARTY_NOTICES.txt"),
      join(outDir, "THIRD_PARTY_NOTICES.txt")
    );
    copyFileSync(
      join(desktopAssetsDir, "fonts", "OFL.txt"),
      join(outDir, "fonts", "OFL.txt")
    );
    copyFileSync(
      join(desktopAssetsDir, "icons", "LICENSE.txt"),
      join(outDir, "icons", "LICENSE.txt")
    );

    console.log(`Built React extension for ${target}`);
  }
}

await runBuild();
