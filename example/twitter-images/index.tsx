import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { type OutputFormat, Renderer } from "@takumi-rs/core";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import * as FiveHundredStars from "./components/500-stars";
import * as OgImage from "./components/og-image";
import * as PackageOgImage from "./components/package-og-image";
import * as PrismaOGImage from "./components/prisma-og-image";
import * as XPostImage from "./components/x-post-image";

const components = [
  OgImage,
  FiveHundredStars,
  XPostImage,
  PrismaOGImage,
  PackageOgImage,
];

type Component = (typeof components)[number];

async function render(
  module: Component,
  ratio = 1,
  format: OutputFormat = "png",
) {
  const { node, stylesheets } = await fromJsx(<module.default />);

  const prepareStart = performance.now();
  const renderer = new Renderer({
    persistentImages: module.persistentImages,
    fonts:
      module.fonts.length > 0
        ? await Promise.all(
            module.fonts.map((font) =>
              readFile(join("../../assets/fonts", font)),
            ),
          )
        : undefined,
  });

  const renderStart = performance.now();

  const buffer = await renderer.render(node, {
    width: module.width * ratio,
    height: module.height * ratio,
    devicePixelRatio: ratio,
    stylesheets,
    drawDebugBorder: process.argv.includes("--debug"),
    format,
  });

  const end = performance.now();

  console.log(
    `Rendered ${module.name} ${ratio}x in ${Math.round(end - prepareStart)}ms (prepare: ${Math.round(renderStart - prepareStart)}ms, render: ${Math.round(end - renderStart)}ms)`,
  );

  const fileName =
    ratio === 1
      ? `${module.name}.${format}`
      : `${module.name}@${ratio}x.${format}`;

  await writeFile(join("output", fileName), buffer);
}

for (const component of components) {
  await render(component);
  await render(component, 2, "webp");
}
