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
import {
  ALIGNS,
  DEFAULT_TEMPLATE_NAME,
  MARGIN_PRESETS,
  PAPERS,
  templateLabel,
} from "../lib/presets.js";
import { pageSetupDefaultsPatch } from "../lib/settings.js";
import { fileBasename } from "../lib/title.js";

const PAGE_SELECTS = [
  { label: "Paper", data: PAPERS, field: "paper" },
  { label: "Margins", data: MARGIN_PRESETS, field: "marginPreset" },
  { label: "Align", data: ALIGNS, field: "textAlign" },
];

/** The `__custom__` Select value stands for an active custom template file. */
const CUSTOM_TEMPLATE = "__custom__";

/** The popover body, mounted while open. Controlled: `options` is the single
 *  source of truth, so the selects always show the effective settings and a
 *  change re-renders the preview at once. `templates` is the bundled template
 *  id list from `list_templates`. */
export function PageSettingsForm({
  options,
  templates = [],
  onOptionChange,
  onPickTemplate,
  onClearTemplate,
}) {
  const customTemplate = options.templatePath;
  const templateData = [
    ...templates.map((id) => ({ value: id, label: templateLabel(id) })),
    ...(customTemplate
      ? [{ value: CUSTOM_TEMPLATE, label: `Custom: ${fileBasename(customTemplate)}` }]
      : []),
  ];
  const templateValue = customTemplate
    ? CUSTOM_TEMPLATE
    : options.templateName ?? DEFAULT_TEMPLATE_NAME;

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
      <Select
        label="Template"
        size="sm"
        data={templateData}
        value={templateValue}
        allowDeselect={false}
        comboboxProps={{ withinPortal: false }}
        onChange={(v) => {
          // Picking the active custom entry again is a no-op; the × button
          // clears it back to the bundled default.
          if (v === CUSTOM_TEMPLATE) return;
          onOptionChange({ templateName: v, templatePath: null });
        }}
      />
      <Group justify="space-between" gap="xs" wrap="nowrap">
        <Text size="xs" c="dimmed" style={{ minWidth: 0 }}>
          Bundled themes ship with the app; a custom file overrides them.
        </Text>
        <Group gap={4} wrap="nowrap">
          <Button size="compact-xs" variant="light" onClick={onPickTemplate}>
            Choose…
          </Button>
          {customTemplate && (
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

/** The badge shown on the Page button when a non-default template is active
 *  (a bundled theme other than light, or a custom file). */
function activeTemplateBadge(options) {
  if (options.templatePath) return fileBasename(options.templatePath);
  const name = options.templateName ?? DEFAULT_TEMPLATE_NAME;
  return name === DEFAULT_TEMPLATE_NAME ? null : templateLabel(name);
}

function PageSettings({ options, templates, onOptionChange, onPickTemplate, onClearTemplate }) {
  const [opened, setOpened] = useState(false);
  const templateName = activeTemplateBadge(options);

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
        {/* The dot marks a non-default template; its tooltip names it without
            opening the popover. Mantine cannot wrap the Popover target in a
            Tooltip, but a Tooltip nested on the inner dot is fine. */}
        <Button
          variant="subtle"
          size="compact-md"
          leftSection={<IconPageBreak size={16} />}
          rightSection={
            templateName ? (
              <Tooltip label={`Template: ${templateName}`}>
                <span
                  className="template-dot"
                  role="img"
                  aria-label={`Template: ${templateName}`}
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
          templates={templates}
          onOptionChange={onOptionChange}
          onPickTemplate={onPickTemplate}
          onClearTemplate={onClearTemplate}
        />
      </Popover.Dropdown>
    </Popover>
  );
}

export default PageSettings;
