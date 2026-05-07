import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { UrlPreview } from "./UrlPreview";

describe("UrlPreview", () => {
  it("renders method and url", () => {
    render(
      <UrlPreview
        kind="url"
        method="GET"
        url="https://api.example.com/v1/users"
        headers={[]}
        body_preview={null}
      />,
    );
    expect(screen.getByText("GET")).toBeInTheDocument();
    expect(screen.getByText("https://api.example.com/v1/users")).toBeInTheDocument();
  });

  it("renders headers", () => {
    render(
      <UrlPreview
        kind="url"
        method="POST"
        url="https://api.example.com/v1/data"
        headers={[["Content-Type", "application/json"]]}
        body_preview='{"key":"value"}'
      />,
    );
    expect(screen.getByText("Content-Type")).toBeInTheDocument();
    expect(screen.getByText("application/json")).toBeInTheDocument();
    expect(screen.getByText('{"key":"value"}')).toBeInTheDocument();
  });
});
