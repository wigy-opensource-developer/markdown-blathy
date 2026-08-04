// Toolbar: file buttons, flavor/paper/margins/align selects, template picker
// and export (§8.4 of SPEC.md). File/template/export handlers arrive in M4/M5.
import { memo } from "react";
import { Button, Divider, Group, Select, Tooltip } from "@mantine/core";
import {
  IconDeviceFloppy,
  IconFilePlus,
  IconFileTypePdf,
  IconFolderOpen,
  IconTemplate,
} from "@tabler/icons-react";
import { ALIGNS, FLAVORS, MARGIN_PRESETS, PAPERS } from "../lib/presets.js";

function Toolbar({
  options,
  onOptionChange,
  onNew,
  onOpen,
  onSave,
  onPickTemplate,
  onExport,
}) {
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
        <Tooltip label="Save Markdown file">
          <Button variant="subtle" size="compact-md" leftSection={<IconDeviceFloppy size={16} />} onClick={onSave}>
            Save
          </Button>
        </Tooltip>
      </Group>

      <Divider orientation="vertical" />

      <Group gap="xs" align="end">
        <Select
          label="Flavor"
          size="sm"
          w={130}
          data={FLAVORS}
          value={options.flavor}
          onChange={(v) => onOptionChange({ flavor: v })}
        />
        <Select
          label="Paper"
          size="sm"
          w={110}
          data={PAPERS}
          value={options.paper}
          onChange={(v) => onOptionChange({ paper: v })}
        />
        <Select
          label="Margins"
          size="sm"
          w={190}
          data={MARGIN_PRESETS}
          value={options.marginPreset}
          onChange={(v) => onOptionChange({ marginPreset: v })}
        />
        <Select
          label="Align"
          size="sm"
          w={110}
          data={ALIGNS}
          value={options.textAlign}
          onChange={(v) => onOptionChange({ textAlign: v })}
        />
      </Group>

      <div className="spacer" />

      <Tooltip label="Choose a custom .typ template">
        <Button variant="subtle" size="compact-md" leftSection={<IconTemplate size={16} />} onClick={onPickTemplate}>
          Template…
        </Button>
      </Tooltip>
      <Button color="blue" size="compact-md" leftSection={<IconFileTypePdf size={16} />} onClick={onExport}>
        Export PDF
      </Button>
    </header>
  );
}

export default memo(Toolbar);
