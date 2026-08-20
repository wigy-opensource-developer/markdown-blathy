// Document settings popover (§6.2, §8.4): edits the front-matter settings —
// title, author, auto heading level, and the Markdown extension flags — and
// writes them back through the Rust front-matter editor (`updateFrontMatter`),
// which preserves any keys this app does not manage. The effective settings
// come from the last render result (`docMeta`), so the frontend never parses
// YAML itself.
import { useRef, useState } from "react";
import {
  Button,
  Checkbox,
  Divider,
  Group,
  NumberInput,
  Popover,
  SimpleGrid,
  Stack,
  Text,
  TextInput,
} from "@mantine/core";
import { IconFileSettings } from "@tabler/icons-react";
import { FEATURES, FEATURE_PRESETS } from "../lib/presets.js";

// The auto heading level is a small integer; a stepper is clearer than a free
// number field. 0 disables auto heading IDs (§6.2).
const HEADING_LEVEL_MIN = 0;
const HEADING_LEVEL_MAX = 6;

function DocumentSettings({ docMeta, onApplySettings }) {
  const [opened, setOpened] = useState(false);

  // Form state, (re)initialized from the latest render result each time the
  // popover opens. `initial` snapshots the opened values so Apply can send
  // only the fields the user actually changed.
  const [title, setTitle] = useState("");
  const [author, setAuthor] = useState("");
  const [autoHeadingLevel, setAutoHeadingLevel] = useState(HEADING_LEVEL_MAX);
  const [features, setFeatures] = useState({ ...FEATURE_PRESETS.github });
  const initial = useRef(null);

  const open = () => {
    const t = docMeta?.title ?? "";
    const a = docMeta?.author ?? "";
    const level = docMeta?.autoHeadingLevel ?? HEADING_LEVEL_MAX;
    // docMeta.features is the effective set once a render has landed; the
    // github preset is the fallback matching the default toolbar flavor.
    const feats = { ...FEATURE_PRESETS.github, ...(docMeta?.features ?? {}) };
    setTitle(t);
    setAuthor(a);
    setAutoHeadingLevel(level);
    setFeatures(feats);
    initial.current = {
      title: t,
      author: a,
      autoHeadingLevel: level,
      features: feats,
    };
    setOpened(true);
  };

  const applyPreset = (name) => setFeatures({ ...FEATURE_PRESETS[name] });

  const apply = async () => {
    const start = initial.current ?? {};
    const patch = {};
    if (title !== start.title) patch.title = title; // "" clears the key
    if (author !== start.author) patch.author = author;
    if (autoHeadingLevel !== start.autoHeadingLevel) {
      patch.autoHeadingLevel = autoHeadingLevel;
    }
    if (JSON.stringify(features) !== JSON.stringify(start.features)) {
      patch.features = features;
    }
    // Skip the round-trip entirely when nothing changed.
    if (Object.keys(patch).length > 0) {
      await onApplySettings(patch);
    }
    setOpened(false);
  };

  return (
    <Popover
      opened={opened}
      onChange={setOpened}
      position="bottom"
      withArrow
      width={420}
      trapFocus
      closeOnEscape
    >
      <Popover.Target>
        <Button
          variant="subtle"
          size="compact-md"
          leftSection={<IconFileSettings size={16} />}
          onClick={opened ? () => setOpened(false) : open}
        >
          Document
        </Button>
      </Popover.Target>
      <Popover.Dropdown>
        <Stack gap="sm">
          <Group grow gap="sm">
            <TextInput
              label="Title"
              size="xs"
              value={title}
              onChange={(e) => setTitle(e.currentTarget.value)}
              placeholder="Untitled"
            />
            <TextInput
              label="Author"
              size="xs"
              value={author}
              onChange={(e) => setAuthor(e.currentTarget.value)}
              placeholder="Anonymous"
            />
          </Group>
          <NumberInput
            label="Auto heading level"
            description="Headings up to this level get an anchor ID (0 = off)."
            size="xs"
            min={HEADING_LEVEL_MIN}
            max={HEADING_LEVEL_MAX}
            value={autoHeadingLevel}
            onChange={(v) => setAutoHeadingLevel(Number(v) || 0)}
          />

          <Divider label="Markdown extensions" labelPosition="left" />
          <SimpleGrid cols={2} spacing={4}>
            {FEATURES.map((f) => (
              <Checkbox
                key={f.key}
                size="xs"
                label={f.label}
                checked={Boolean(features[f.key])}
                onChange={(e) => {
                  // Read the DOM value NOW: React nulls `e.currentTarget`
                  // once the handler returns, and a functional setState
                  // updater runs after that — reading it in there throws.
                  const checked = e.currentTarget.checked;
                  setFeatures((prev) => ({ ...prev, [f.key]: checked }));
                }}
              />
            ))}
          </SimpleGrid>

          <Group gap="xs">
            <Text size="xs" c="dimmed">
              Presets:
            </Text>
            <Button size="compact-xs" variant="light" onClick={() => applyPreset("commonmark")}>
              CommonMark
            </Button>
            <Button size="compact-xs" variant="light" onClick={() => applyPreset("github")}>
              GitHub
            </Button>
            <Button size="compact-xs" variant="light" onClick={() => applyPreset("custom")}>
              Custom
            </Button>
          </Group>

          <Divider />
          <Group justify="flex-end" gap="xs">
            <Button size="compact-sm" variant="default" onClick={() => setOpened(false)}>
              Cancel
            </Button>
            <Button size="compact-sm" color="blue" onClick={apply}>
              Apply
            </Button>
          </Group>
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
}

export default DocumentSettings;
