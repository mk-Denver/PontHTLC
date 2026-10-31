import { renderSVG } from "uqr";

export function qrSvg(text: string): string {
  return renderSVG(text, { ecc: "M", pixelSize: 1, whiteColor: "#ffffff", blackColor: "#1b1712" });
}
