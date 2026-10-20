import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { Segmented } from "./Segmented";

function Demo() {
  const [v, setV] = useState<"a" | "b" | "c">("a");
  return (
    <Segmented
      variant="pill"
      label="Choice"
      options={["a", "b", "c"] as const}
      value={v}
      onChange={setV}
      optionLabel={(o) => o.toUpperCase()}
    />
  );
}

describe("Segmented", () => {
  it("is a radio group where arrows, Home and End move the choice", () => {
    render(<Demo />);
    const radio = (name: string) => screen.getByRole("radio", { name });
    expect(radio("A")).toHaveProperty("tabIndex", 0);
    expect(radio("B")).toHaveProperty("tabIndex", -1);
    fireEvent.keyDown(radio("A"), { key: "ArrowRight" });
    expect(radio("B").getAttribute("aria-checked")).toBe("true");
    fireEvent.keyDown(radio("B"), { key: "End" });
    expect(radio("C").getAttribute("aria-checked")).toBe("true");
    fireEvent.keyDown(radio("C"), { key: "ArrowRight" });
    expect(radio("A").getAttribute("aria-checked")).toBe("true");
    fireEvent.keyDown(radio("A"), { key: "ArrowLeft" });
    expect(radio("C").getAttribute("aria-checked")).toBe("true");
    fireEvent.click(radio("B"));
    expect(radio("B").getAttribute("aria-checked")).toBe("true");
  });
});
