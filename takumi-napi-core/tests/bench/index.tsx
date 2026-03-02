import { writeFile } from "node:fs/promises";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import { Globe2 } from "lucide-react";
import { bench, run, summary } from "mitata";
import DocsTemplate from "../../../takumi-template/src/templates/docs-template";
import { Renderer } from "../../index.js";

function createNode() {
  return fromJsx(
    <DocsTemplate
      title="Takumi Benchmark"
      description="See how Takumi performs in real world use cases!"
      site="takumi.kane.tw"
      icon={<Globe2 size={64} color="white" />}
      primaryColor="blue"
      primaryTextColor="white"
    />,
  );
}

async function createAnimationNodes() {
  const fps = 30;
  const durationMs = 1000;
  const totalFrames = (durationMs * fps) / 1000;

  const frames = await Array.fromAsync({ length: totalFrames }, async () => {
    const { node, stylesheets } = await createNode();
    return {
      node,
      durationMs: durationMs / totalFrames,
      stylesheets,
    };
  });

  return {
    frames,
    fps,
    durationMs,
  };
}

const renderer = new Renderer();

bench("createNode", createNode);

summary(() => {
  bench("createNode + render (raw)", async () => {
    const { node, stylesheets } = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      format: "raw",
      stylesheets,
    });
  });

  bench("createNode + render (png, fdeflate)", async () => {
    const { node, stylesheets } = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      quality: 75,
      stylesheets,
    });
  });

  bench("createNode + render (png, flate2)", async () => {
    const { node, stylesheets } = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      quality: 100,
      stylesheets,
    });
  });

  bench("createNode + render (webp 75%)", async () => {
    const { node, stylesheets } = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      format: "webp",
      quality: 75,
      stylesheets,
    });
  });

  bench("createNode + render (webp 100%)", async () => {
    const { node, stylesheets } = await createNode();
    return renderer.render(node, {
      width: 1200,
      height: 630,
      format: "webp",
      quality: 100,
      stylesheets,
    });
  });
});

summary(() => {
  bench("createNode + renderAnimation (webp, 30fps, 1000ms)", async () => {
    const { frames, fps, durationMs } = await createAnimationNodes();

    if (fps !== 30 || durationMs !== 1000) {
      throw new Error("Invalid fps or durationMs");
    }

    return renderer.renderAnimation(frames, {
      width: 1200,
      height: 630,
      format: "webp",
    });
  });

  bench("createNode + renderAnimation (apng, 30fps, 1000ms)", async () => {
    const { frames, fps, durationMs } = await createAnimationNodes();

    if (fps !== 30 || durationMs !== 1000) {
      throw new Error("Invalid fps or durationMs");
    }

    return renderer.renderAnimation(frames, {
      width: 1200,
      height: 630,
      format: "apng",
    });
  });
});

const { node, stylesheets } = await createNode();

await writeFile(
  "tests/bench/bench.png",
  await renderer.render(node, {
    width: 1200,
    height: 630,
    stylesheets,
  }),
);

await run();
