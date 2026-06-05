// Convert hardcoded TS app templates into JSON seed files for the backend.
// Run via: npx ts-node frontend/scripts/dump-templates.ts
// Output: backend/src/templates/seed/*.json (one file per template) + index.json

import * as fs from "fs";
import * as path from "path";
import { allTemplates, databaseTemplates } from "../src/shared/templates/all.templates";

const outDir = path.resolve(__dirname, "../../backend/src/templates/seed");
fs.mkdirSync(outDir, { recursive: true });

const slugify = (name: string) =>
    name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "");

const dbSlugs = new Set(databaseTemplates.map(t => slugify(t.name)));

interface SeedEntry {
    slug: string;
    name: string;
    icon_url: string | null;
    category: "app" | "database";
    image_repository: string;   // e.g. "library/adminer", "mysql"
    image_tag: string;          // e.g. "latest", "8.4"
    spec: any;
    inputs: any[];              // inputSettings WITHOUT the image entry
}

// Parse "[registry/]repo[:tag]" into { repo, tag }.
// We only split the LAST ':' because registries can have a port (host:port/repo).
function parseImageRef(raw: string): { repo: string; tag: string } {
    if (!raw) return { repo: "", tag: "latest" };
    const slash = raw.lastIndexOf("/");
    const lastSegmentStart = slash + 1;
    const colon = raw.indexOf(":", lastSegmentStart);
    if (colon < 0) return { repo: raw, tag: "latest" };
    return { repo: raw.slice(0, colon), tag: raw.slice(colon + 1) || "latest" };
}

const seed: SeedEntry[] = [];

for (const t of allTemplates) {
    const slug = slugify(t.name);
    if (!t.templates || t.templates.length === 0) continue;
    const first = t.templates[0];

    const inputs = first.inputSettings ?? [];
    const imageEntry = inputs.find((i: any) => i.key === "containerImageSource");
    const { repo, tag } = parseImageRef(imageEntry?.value ?? "");
    const inputsWithoutImage = inputs.filter(
        (i: any) => i.key !== "containerImageSource",
    );

    seed.push({
        slug,
        name: t.name,
        icon_url: t.iconName ?? null,
        category: dbSlugs.has(slug) ? "database" : "app",
        image_repository: repo,
        image_tag: tag,
        spec: {
            appModel: first.appModel,
            appDomains: first.appDomains ?? [],
            appVolumes: first.appVolumes ?? [],
            appFileMounts: first.appFileMounts ?? [],
            appPorts: first.appPorts ?? [],
        },
        inputs: inputsWithoutImage,
    });
}

seed.sort((a, b) => a.slug.localeCompare(b.slug));

for (const entry of seed) {
    const file = path.join(outDir, `${entry.slug}.json`);
    fs.writeFileSync(file, JSON.stringify(entry, null, 2) + "\n");
}

// Index lists every slug shipped — used by the backend startup loader.
fs.writeFileSync(
    path.join(outDir, "index.json"),
    JSON.stringify(seed.map(e => e.slug), null, 2) + "\n",
);

console.log(`wrote ${seed.length} template seed files to ${outDir}`);
