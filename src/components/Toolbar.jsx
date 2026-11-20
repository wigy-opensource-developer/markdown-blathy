// Toolbar: file buttons and the Document settings popover on the left (the
// authoring side); the Page settings popover and Export PDF on the right (the
// typesetting side). Paper/margins/align and the template all live inside the
// Page popover (§8.4, §8.6 of SPEC.md), so the toolbar only carries the small
// custom-template dot on its button.
//
// Buttons dispatch command ids through the registry; labels, tooltips and
// `aria-keyshortcuts` come from the same descriptors as the keyboard handler.
import { memo } from "react";
import { Button, Group, Tooltip } from "@mantine/core";
import {
  IconDeviceFloppy,
  IconFilePlus,
  IconFileTypePdf,
  IconFolderOpen,
  IconLink,
  IconLinkOff,
} from "@tabler/icons-react";
import {
  ariaKeyShortcut,
  commandTooltip,
  getCommand,
} from "../lib/commands.js";
import DocumentSettings from "./DocumentSettings.jsx";
import PageSettings from "./PageSettings.jsx";
import ThemeMenu from "./ThemeMenu.jsx";

function Toolbar({
  platform,
  dispatch,
  options,
  templates,
  theme,
  onThemeChange,
  onOptionChange,
  onPickTemplate,
  onClearTemplate,
  docMeta,
  onApplySettings,
  documentSettingsOpen,
  onDocumentSettingsOpenChange,
  pageSettingsOpen,
  onPageSettingsOpenChange,
  themeMenuOpen,
  onThemeMenuOpenChange,
  syncScroll,
  onToggleSyncScroll,
}) {
  const tip = (id) => commandTooltip(getCommand(id), platform);
  const aria = (id) => ariaKeyShortcut(getCommand(id), platform);

  return (
    <header id="toolbar">
      <Group gap="xs">
        <Tooltip label={tip("file.new")}>
          <Button
            variant="subtle"
            size="compact-md"
            leftSection={<IconFilePlus size={16} />}
            aria-keyshortcuts={aria("file.new")}
            data-command="file.new"
            onClick={() => dispatch("file.new")}
          >
            New
          </Button>
        </Tooltip>
        <Tooltip label={tip("file.open")}>
          <Button
            variant="subtle"
            size="compact-md"
            leftSection={<IconFolderOpen size={16} />}
            aria-keyshortcuts={aria("file.open")}
            data-command="file.open"
            onClick={() => dispatch("file.open")}
          >
            Open
          </Button>
        </Tooltip>
        <Tooltip label={`${tip("file.save")} · ${tip("file.saveAs")}`}>
          <Button
            variant="subtle"
            size="compact-md"
            leftSection={<IconDeviceFloppy size={16} />}
            aria-keyshortcuts={aria("file.save")}
            data-command="file.save"
            onClick={() => dispatch("file.save")}
          >
            Save
          </Button>
        </Tooltip>
        {/* Not wrapped in a Tooltip on purpose: Mantine cannot wrap another
            floating component (the settings Popover) in a Tooltip. */}
        <DocumentSettings
          docMeta={docMeta}
          onApplySettings={onApplySettings}
          opened={documentSettingsOpen}
          onOpenChange={onDocumentSettingsOpenChange}
          shortcut={tip("document.settings")}
          ariaShortcut={aria("document.settings")}
        />
      </Group>

      <div className="spacer" />

      {/* Centered between two spacers: the scroll-sync toggle belongs to
          neither the authoring (left) nor the typesetting (right) group.
          Clicking it returns focus to the editor (App.jsx) so typing is
          never interrupted. */}
      <Tooltip label={syncScroll ? "Disable synchronized scrolling" : "Enable synchronized scrolling"}>
        <Button
          variant={syncScroll ? "light" : "subtle"}
          size="compact-md"
          leftSection={
            syncScroll ? (
              <IconLink size={16} />
            ) : (
              <IconLinkOff size={16} />
            )
          }
          aria-label={syncScroll ? "Disable synchronized scrolling" : "Enable synchronized scrolling"}
          aria-pressed={syncScroll}
          onClick={onToggleSyncScroll}
        >
          Sync scroll
        </Button>
      </Tooltip>

      <div className="spacer" />

      {/* UI appearance (light / system / dark) — skins the app chrome only,
          never the document or the exported PDF (§8.6). */}
      <ThemeMenu
        theme={theme}
        onThemeChange={onThemeChange}
        opened={themeMenuOpen}
        onOpenChange={onThemeMenuOpenChange}
      />

      <PageSettings
        options={options}
        templates={templates}
        onOptionChange={onOptionChange}
        onPickTemplate={onPickTemplate}
        onClearTemplate={onClearTemplate}
        opened={pageSettingsOpen}
        onOpenChange={onPageSettingsOpenChange}
        shortcut={tip("page.settings")}
        ariaShortcut={aria("page.settings")}
      />
      <Tooltip label={tip("file.exportPdf")}>
        <Button
          color="blue"
          size="compact-md"
          leftSection={<IconFileTypePdf size={16} />}
          aria-keyshortcuts={aria("file.exportPdf")}
          data-command="file.exportPdf"
          onClick={() => dispatch("file.exportPdf")}
        >
          Export PDF
        </Button>
      </Tooltip>
    </header>
  );
}

export default memo(Toolbar);
