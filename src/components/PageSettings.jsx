// Page settings popover (§8.6 of SPEC.md): paper, margins, alignment and the
// Typst template. These are global preferences (localStorage), not
// per-document front matter — the scope line says so. The popover is
// deliberately NOT modal: unlike the Document popover it neither blurs nor
// freezes the panes, and every change applies immediately, so the WYSIWYG
// preview behind it is the feedback.
//
// The Selects keep their dropdowns out of portals (`withinPortal: false`): a
// portaled dropdown counts as an outside click and would close the popover on
// the first selection (the Document popover works around the same Mantine
// behavior).
import { useState } from "react";
import {
  Button,
  Divider,
  Group,
  Popover,
  Select,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import { IconPageBreak } from "@tabler/icons-react";
import { ALIGNS, MARGIN_PRESETS, PAPERS } from "../lib/presets.js";
import { pageSetupDefaultsPatch } from "../lib/settings.js";
import { fileBasename } from "../lib/title.js";

const PAGE_SELECTS = [
  { label: "Paper", data: PAPERS, field: "paper" },
  { label: "Margins", data: MARGIN_PRESETS, field: "marginPreset" },
  { label: "Align", data: ALIGNS, field: "textAlign" },
];

/** The popover body, mounted while open. Controlled: `options` is the single
 *  source of truth, so the selects always show the effective settings and a
 *  change re-renders the preview at once. */
export function PageSettingsForm({
  options,
  onOptionChange,
  onPickTemplate,
  onClearTemplate,
}) {
  const template = options.templatePath;
  return (
    <Stack gap="sm">
      <Text size="xs" c="dimmed">
        Applies to all documents on this device.
      </Text>
      {PAGE_SELECTS.map(({ label, data, field }) => (
        <Select
          key={field}
          label={label}
          size="sm"
          data={data}
          value={options[field]}
          comboboxProps={{ withinPortal: false }}
          onChange={(v) => onOptionChange({ [field]: v })}
        />
      ))}

      <Divider label="Template" labelPosition="left" />
      <Group justify="space-between" gap="xs" wrap="nowrap">
        <Text size="sm" truncate style={{ minWidth: 0 }} title={template ?? undefined}>
          {template ? fileBasename(template) : "Embedded default"}
        </Text>
        <Group gap={4} wrap="nowrap">
          <Button size="compact-xs" variant="light" onClick={onPickTemplate}>
            Choose…
          </Button>
          {template && (
            <Button
              size="compact-xs"
              variant="subtle"
              aria-label="Reset custom template"
              onClick={onClearTemplate}
            >
              ×
            </Button>
          )}
        </Group>
      </Group>

      <Divider />
      <Group justify="flex-end">
        <Button
          size="compact-sm"
          variant="default"
          onClick={() => onOptionChange(pageSetupDefaultsPatch())}
        >
          Restore defaults
        </Button>
      </Group>
    </Stack>
  );
}

function PageSettings({ options, onOptionChange, onPickTemplate, onClearTemplate }) {
  const [opened, setOpened] = useState(false);
  const templateName = options.templatePath
    ? fileBasename(options.templatePath)
    : null;

  return (
    <Popover
      opened={opened}
      onChange={setOpened}
      position="bottom"
      withArrow
      width={340}
      closeOnEscape
    >
      <Popover.Target>
        {/* The dot marks a custom template; its tooltip names it without
            opening the popover. Mantine cannot wrap the Popover target in a
            Tooltip, but a Tooltip nested on the inner dot is fine. */}
        <Button
          variant="subtle"
          size="compact-md"
          leftSection={<IconPageBreak size={16} />}
          rightSection={
            templateName ? (
              <Tooltip label={`Custom template: ${templateName}`}>
                <span
                  className="template-dot"
                  role="img"
                  aria-label={`Custom template: ${templateName}`}
                />
              </Tooltip>
            ) : null
          }
          onClick={() => setOpened((open) => !open)}
        >
          Page…
        </Button>
      </Popover.Target>
      <Popover.Dropdown className="app-popover">
        <PageSettingsForm
          options={options}
          onOptionChange={onOptionChange}
          onPickTemplate={onPickTemplate}
          onClearTemplate={onClearTemplate}
        />
      </Popover.Dropdown>
    </Popover>
  );
}

export default PageSettings;
