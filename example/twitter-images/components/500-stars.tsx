import { readFile } from "node:fs/promises";

export const persistentImages = [
  {
    src: "takumi.svg",
    data: await readFile("../../assets/images/takumi.svg"),
  },
];

export const name = "500-stars";

export const width = 1200;
export const height = 675;

export const fonts = [
  "plus-jakarta-sans/PlusJakartaSans-VariableFont_wght.woff2",
];

export default function FiveHundredStars() {
  return (
    <div
      style={{
        backgroundImage:
          "radial-gradient(circle at center bottom, rgba(227, 179, 65, 1.0), rgba(227, 179, 65, 0.0) 75%)",
        backgroundColor: "black",
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <img
        src={persistentImages[0]?.src}
        alt="Logo"
        style={{
          width: "12rem",
          aspectRatio: 1,
          marginBottom: "2rem",
        }}
      />
      <span
        style={{
          color: "rgba(255, 255, 255, 0.85)",
          fontSize: "8rem",
          fontWeight: 800,
          padding: "1rem 0",
        }}
      >
        500 Stars
      </span>
      <span
        style={{
          color: "rgba(255, 255, 255, 0.75)",
          fontSize: "2rem",
          fontWeight: 600,
          padding: "1rem 0",
        }}
      >
        Background noise will be in the next version of Takumi.
      </span>
      <div
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          width: "100%",
          height: "100%",
          backgroundImage: "noise-v1(opacity(0.2))",
        }}
      />
    </div>
  );
}
