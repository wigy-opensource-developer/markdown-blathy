// Toolbar: file buttons and the Document settings popover on the left (the
// authoring side); the Page settings popover and Export PDF on the right (the
// typesetting side). Paper/margins/align and the template all live inside the
// Page popover (§8.4, §8.6 of SPEC.md), so the toolbar only carries the small
// custom-template dot on its button.
import { memo } from "react";
import { Button, Group, Tooltip } from "@mantine/core";
import {
  IconDeviceFloppy,
  IconFilePlus,
  IconFileTypePdf,
  IconFolderOpen,
} from "@tabler/icons-react";
import DocumentSettings from "./DocumentSettings.jsx";
import PageSettings from "./PageSettings.jsx";

function Toolbar({
  options,
  onOptionChange,
  onNew,
  onOpen,
  onSave,
  onPickTemplate,
  onClearTemplate,
  onExport,
  docMeta,
  onApplySettings,
  onSettingsOpenChange,
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
        <Tooltip label="Save Markdown As…">
          <Button variant="subtle" size="compact-md" leftSection={<IconDeviceFloppy size={16} />} onClick={onSave}>
            Save
          </Button>
        </Tooltip>
        {/* Not wrapped in a Tooltip on purpose: Mantine cannot wrap another
            floating component (the settings Popover) in a Tooltip. */}
        <DocumentSettings
          docMeta={docMeta}
          onApplySettings={onApplySettings}
          onOpenChange={onSettingsOpenChange}
        />
      </Group>

      <div className="spacer" />

      <PageSettings
        options={options}
        onOptionChange={onOptionChange}
        onPickTemplate={onPickTemplate}
        onClearTemplate={onClearTemplate}
      />
      <Button color="blue" size="compact-md" leftSection={<IconFileTypePdf size={16} />} onClick={onExport}>
        Export PDF
      </Button>
    </header>
  );
}

export default memo(Toolbar);
