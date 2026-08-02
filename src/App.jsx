import { useState } from "react";
import {
  Button,
  Divider,
  Group,
  Paper,
  Select,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  IconDeviceFloppy,
  IconFileCode,
  IconFilePlus,
  IconFileTypePdf,
  IconFolderOpen,
  IconTemplate,
} from "@tabler/icons-react";

const INITIAL_DOC = `# Hello, Markdown editor

Start typing in the editor pane. The live preview and PDF export are
powered by a Typst compilation pipeline running on the Rust side.
`;

const FLAVORS = [
  { value: "commonmark", label: "CommonMark" },
  { value: "gfm", label: "GFM" },
  { value: "custom", label: "Custom" },
];

const PAPERS = [
  { value: "a4", label: "A4" },
  { value: "a5", label: "A5" },
  { value: "letter", label: "Letter" },
  { value: "legal", label: "Legal" },
];

const MARGIN_PRESETS = [
  { value: "normal", label: "Normal (20/25mm)" },
  { value: "narrow", label: "Narrow (10/15mm)" },
  { value: "custom", label: "Custom…" },
];

const ALIGNS = [
  { value: "left", label: "Left" },
  { value: "center", label: "Center" },
  { value: "justified", label: "Justified" },
];

export default function App() {
  const [markdown, setMarkdown] = useState(INITIAL_DOC);
  const [flavor, setFlavor] = useState("gfm");
  const [paper, setPaper] = useState("a4");
  const [margins, setMargins] = useState("normal");
  const [align, setAlign] = useState("left");

  return (
    <div className="app">
      <header id="toolbar">
        <Group gap="xs">
          <Tooltip label="New document">
            <Button variant="subtle" size="compact-sm" leftSection={<IconFilePlus size={16} />}>
              New
            </Button>
          </Tooltip>
          <Tooltip label="Open Markdown file">
            <Button variant="subtle" size="compact-sm" leftSection={<IconFolderOpen size={16} />}>
              Open
            </Button>
          </Tooltip>
          <Tooltip label="Save Markdown file">
            <Button variant="subtle" size="compact-sm" leftSection={<IconDeviceFloppy size={16} />}>
              Save
            </Button>
          </Tooltip>
        </Group>

        <Divider orientation="vertical" />

        <Group gap="xs" align="end">
          <Select label="Flavor" size="xs" value={flavor} onChange={setFlavor} data={FLAVORS} />
          <Select label="Paper" size="xs" value={paper} onChange={setPaper} data={PAPERS} />
          <Select label="Margins" size="xs" value={margins} onChange={setMargins} data={MARGIN_PRESETS} />
          <Select label="Align" size="xs" value={align} onChange={setAlign} data={ALIGNS} />
        </Group>

        <div className="spacer" />

        <Tooltip label="Choose a custom .typ template">
          <Button variant="subtle" size="compact-sm" leftSection={<IconTemplate size={16} />}>
            Template…
          </Button>
        </Tooltip>
        <Button color="blue" size="compact-sm" leftSection={<IconFileTypePdf size={16} />}>
          Export PDF
        </Button>
      </header>

      <main id="panes">
        <section id="editor-pane">
          <textarea
            id="editor"
            className="editor"
            value={markdown}
            onChange={(e) => setMarkdown(e.currentTarget.value)}
            spellCheck={false}
          />
        </section>
        <section id="preview-pane">
          <Paper id="preview" shadow="md" p="lg">
            <Text size="sm" c="dimmed">
              Live preview renders here — per-page SVG from the Typst pipeline.
            </Text>
            <Text size="sm" c="dimmed" mt="xs">
              <IconFileCode size={14} style={{ verticalAlign: "-2px" }} /> The preview and the
              exported PDF are produced by the same compilation pipeline (true WYSIWYG).
            </Text>
          </Paper>
        </section>
      </main>

      <footer id="status">
        <Text size="xs" c="dimmed">
          Ready
        </Text>
      </footer>
    </div>
  );
}
