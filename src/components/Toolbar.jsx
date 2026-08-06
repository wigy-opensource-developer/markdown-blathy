// Toolbar: file buttons, flavor/paper/margins/align selects, template picker
// and export (§8.4 of SPEC.md). File/template/export handlers arrive in M4/M5.
import { memo } from "react";
import { Badge, Button, Divider, Group, Select, Tooltip } from "@mantine/core";
import {
  IconDeviceFloppy,
  IconFilePlus,
  IconFileTypePdf,
  IconFolderOpen,
  IconTemplate,
  IconTemplateOff,
} from "@tabler/icons-react";
import { ALIGNS, FLAVORS, MARGIN_PRESETS, PAPERS } from "../lib/presets.js";

// The four toolbar option selects differ only in label, option table, width
// and which options field they write; a config row + one component removes
// the copy-pasted <Select> blocks.
const OPTION_SELECTS = [
  { label: "Flavor", data: FLAVORS, width: 130, field: "flavor" },
  { label: "Paper", data: PAPERS, width: 110, field: "paper" },
  { label: "Margins", data: MARGIN_PRESETS, width: 190, field: "marginPreset" },
  { label: "Align", data: ALIGNS, width: 110, field: "textAlign" },
];

/** A single toolbar option select: writes `{ [field]: value }` up to the app. */
function OptionSelect({ label, data, width, field, value, onOptionChange }) {
  return (
    <Select
      label={label}
      size="sm"
      w={width}
      data={data}
      value={value}
      onChange={(v) => onOptionChange({ [field]: v })}
    />
  );
}

function Toolbar({
  options,
  onOptionChange,
  onNew,
  onOpen,
  onSave,
  onPickTemplate,
  onClearTemplate,
  onExport,
}) {
  const customTemplate = options.templatePath;
  return (
    <header id="toolbar">
      <Group gap="xs">
        <Tooltip label="New document">
          <Button variant="subtle" size="compact-md" leftSection={<IconFilePlus size={16} />} onClick={onNew}>
            New
          </Button>
        </Tooltip>
        <Tooltip label="Open Markdown file">
          <Button variant="subtle" size="compact-md" leftSection={<IconFolderOpen size={16} />} onClick={onOpen}>
            Open
          </Button>
        </Tooltip>
        <Tooltip label="Save Markdown As…">
          <Button variant="subtle" size="compact-md" leftSection={<IconDeviceFloppy size={16} />} onClick={onSave}>
            Save
          </Button>
        </Tooltip>
      </Group>

      <Divider orientation="vertical" />

      <Group gap="xs" align="end">
        {OPTION_SELECTS.map((select) => (
          <OptionSelect
            key={select.field}
            {...select}
            value={options[select.field]}
            onOptionChange={onOptionChange}
          />
        ))}
      </Group>

      <div className="spacer" />

      <Group gap="xs" align="center">
        {customTemplate && (
          <Tooltip label={`Remove custom template (${customTemplate})`}>
            <Badge
              variant="light"
              color="teal"
              leftSection={<IconTemplateOff size={12} />}
              onClick={onClearTemplate}
              style={{ cursor: "pointer" }}
            >
              Custom template
            </Badge>
          </Tooltip>
        )}
        <Tooltip
          label={
            customTemplate
              ? "Replace the custom .typ template"
              : "Choose a custom .typ template"
          }
        >
          <Button
            variant={customTemplate ? "light" : "subtle"}
            size="compact-md"
            leftSection={
              customTemplate ? <IconTemplateOff size={16} /> : <IconTemplate size={16} />
            }
            onClick={onPickTemplate}
          >
            Template…
          </Button>
        </Tooltip>
      </Group>
      <Button color="blue" size="compact-md" leftSection={<IconFileTypePdf size={16} />} onClick={onExport}>
        Export PDF
      </Button>
    </header>
  );
}

export default memo(Toolbar);
