// Document settings popover (§6.2, §8.4): edits the front-matter settings —
// title, author, auto heading level, language/hyphenation, and the Markdown
// extension flags — and writes them back through the Rust front-matter editor
// (`updateFrontMatter`), which preserves any keys this app does not manage.
// The effective settings come from the last render result (`docMeta`), so the
// frontend never parses YAML itself.
//
// The draft is UNCONTROLLED and read from the DOM on Apply. This is not a
// style choice: any React state update while the popover is open commits DOM
// writes that make WebKit run a full-document pass — measured at ~800 ms per
// keystroke against a multi-page SVG preview — whereas an uncontrolled input
// keeps typing in the engine (~30 ms). So no handler in here calls setState:
// text inputs use `defaultValue`, the segmented control and checkboxes are
// DOM-driven, and presets poke the checkbox DOM directly. Only Apply builds
// the patch (diffing the DOM values against the mount-time snapshot) and the
// parent closes the popover.
//
// The controls must also keep their own popups out of portals (hence the
// inline SegmentedControl rather than a Select): a portaled dropdown counts
// as an outside click and would close this Popover mid-edit.
import { useEffect, useRef, useState } from "react";
import {
  Button,
  Checkbox,
  Divider,
  Group,
  Input,
  NumberInput,
  Popover,
  SegmentedControl,
  SimpleGrid,
  Stack,
  Text,
  TextInput,
} from "@mantine/core";
import { IconFileSettings } from "@tabler/icons-react";
import {
  buildDocSettingsPatch,
  hyphenateFormValue,
} from "../lib/docSettings.js";
import { FEATURES, FEATURE_PRESETS } from "../lib/presets.js";

// The auto heading level is a small integer; a stepper is clearer than a free
// number field. 0 disables auto heading IDs (§6.2).
const HEADING_LEVEL_MIN = 0;
const HEADING_LEVEL_MAX = 6;

// Hyphenation control: `auto` clears the front-matter key (Typst hyphenates
// justified text); on/off write the boolean. SegmentedControl renders inline
// — no portal, so clicking an option cannot close the surrounding popover.
const HYPHENATE_OPTIONS = [
  { value: "auto", label: "Auto" },
  { value: "true", label: "On" },
  { value: "false", label: "Off" },
];

/**
 * The popover's form body, mounted only while the popover is open. Everything
 * is uncontrolled: `initial` is the mount-time snapshot and `readForm` pulls
 * the draft back out of the DOM, so interacting with the form never
 * re-renders React (see the file header for why that matters).
 */
function DocumentSettingsForm({ docMeta, onApply, onClose }) {
  const rootRef = useRef(null);
  const titleRef = useRef(null);
  const authorRef = useRef(null);
  const headingLevelRef = useRef(null);
  const langRef = useRef(null);

  const initial = useRef(null);
  if (initial.current === null) {
    initial.current = {
      title: docMeta?.title ?? "",
      author: docMeta?.author ?? "",
      autoHeadingLevel: docMeta?.autoHeadingLevel ?? HEADING_LEVEL_MAX,
      // Empty means Typst's default (English); `auto` means no hyphenate key.
      lang: docMeta?.lang ?? "",
      hyphenate: hyphenateFormValue(docMeta?.hyphenate),
      // docMeta.features is the effective set once a render has landed; the
      // github preset is the fallback matching the default toolbar flavor.
      features: { ...FEATURE_PRESETS.github, ...(docMeta?.features ?? {}) },
    };
  }

  /** The current draft, read from the DOM (the form is uncontrolled). */
  const readForm = () => {
    const root = rootRef.current;
    const features = {};
    // Checkboxes render in FEATURES order; their `value` is the feature key.
    for (const box of root.querySelectorAll('input[type="checkbox"]')) {
      features[box.value] = box.checked;
    }
    return {
      title: titleRef.current?.value ?? "",
      author: authorRef.current?.value ?? "",
      autoHeadingLevel: Number(headingLevelRef.current?.value) || 0,
      lang: langRef.current?.value ?? "",
      hyphenate:
        root.querySelector('input[type="radio"]:checked')?.value ?? "auto",
      features,
    };
  };

  /** Presets poke the checkbox DOM directly — still no React update. */
  const applyPreset = (name) => {
    const preset = FEATURE_PRESETS[name];
    for (const box of rootRef.current.querySelectorAll('input[type="checkbox"]')) {
      box.checked = Boolean(preset[box.value]);
    }
  };

  const apply = async () => {
    const patch = buildDocSettingsPatch(readForm(), initial.current);
    // Skip the round-trip entirely when nothing changed.
    if (Object.keys(patch).length > 0) {
      // Await BEFORE closing: the change is recorded as owed while the
      // freeze is still paused, so `close()` supersedes any stashed render
      // with the single catch-up. Closing first would show the stash and
      // then re-render — two heavy commits instead of one.
      await onApply(patch);
    }
    onClose();
  };

  return (
    <Stack ref={rootRef} gap="sm">
      <Group grow gap="sm">
        <TextInput
          ref={titleRef}
          label="Title"
          size="xs"
          defaultValue={initial.current.title}
          placeholder="Untitled"
        />
        <TextInput
          ref={authorRef}
          label="Author"
          size="xs"
          defaultValue={initial.current.author}
          placeholder="Anonymous"
        />
      </Group>
      <NumberInput
        ref={headingLevelRef}
        label="Auto heading level"
        description="Headings up to this level get an anchor ID (0 = off)."
        size="xs"
        min={HEADING_LEVEL_MIN}
        max={HEADING_LEVEL_MAX}
        defaultValue={initial.current.autoHeadingLevel}
      />

      <Group grow gap="sm" align="flex-start">
        <TextInput
          ref={langRef}
          label="Language"
          description="ISO 639 code; empty = Typst's default (English)."
          size="xs"
          defaultValue={initial.current.lang}
          placeholder="hu"
        />
        <Input.Wrapper
          label="Hyphenation"
          description="Auto hyphenates justified text."
          size="xs"
        >
          <SegmentedControl
            size="xs"
            fullWidth
            data={HYPHENATE_OPTIONS}
            defaultValue={initial.current.hyphenate}
          />
        </Input.Wrapper>
      </Group>

      <Divider label="Markdown extensions" labelPosition="left" />
      <SimpleGrid cols={2} spacing={4}>
        {FEATURES.map((f) => (
          <Checkbox
            key={f.key}
            value={f.key}
            size="xs"
            label={f.label}
            defaultChecked={Boolean(initial.current.features[f.key])}
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
        <Button size="compact-sm" variant="default" onClick={onClose}>
          Cancel
        </Button>
        <Button size="compact-sm" color="blue" onClick={apply}>
          Apply
        </Button>
      </Group>
    </Stack>
  );
}

function DocumentSettings({ docMeta, onApplySettings, onOpenChange }) {
  const [opened, setOpened] = useState(false);

  // Tell the app when the popover opens/closes: it blurs and freezes the
  // document while open, so edits here only re-render the popup; the document
  // catches up once on close (§6.2, lib/previewFreeze.js).
  useEffect(() => {
    onOpenChange?.(opened);
  }, [opened, onOpenChange]);

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
          onClick={() => setOpened((open) => !open)}
        >
          Document
        </Button>
      </Popover.Target>
      <Popover.Dropdown>
        {/* Mounted only while open: fresh draft + snapshot on every open. */}
        {opened && (
          <DocumentSettingsForm
            docMeta={docMeta}
            onApply={onApplySettings}
            onClose={() => setOpened(false)}
          />
        )}
      </Popover.Dropdown>
    </Popover>
  );
}

export default DocumentSettings;
