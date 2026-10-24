// The toolbar theme control is a single icon button that opens a menu; its
// accessible name must reflect the current scheme (an icon alone is not
// enough for screen readers).
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import ThemeMenu from "./ThemeMenu.jsx";

const render = (props) =>
  renderToStaticMarkup(
    <MantineProvider>
      <ThemeMenu theme="system" onThemeChange={() => {}} {...props} />
    </MantineProvider>
  );

describe("ThemeMenu", () => {
  it("names the current theme in the accessible label", () => {
    expect(render()).toContain('aria-label="Color theme: System"');
    expect(render({ theme: "light" })).toContain('aria-label="Color theme: Light"');
    expect(render({ theme: "dark" })).toContain('aria-label="Color theme: Dark"');
  });

  it("renders an icon (svg) for the collapsed control", () => {
    expect(render()).toContain("<svg");
  });
});
