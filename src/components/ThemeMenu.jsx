// UI appearance control (light / system / dark) for the toolbar (§8.6 of
// SPEC.md). A single icon button reflecting the current scheme opens a menu
// whose entries pair the theme icon with its label — the label alone ("System")
// does not signal "UI theming" to most users until the menu is open.
import { ActionIcon, Menu } from "@mantine/core";
import { IconMoon, IconSun, IconSunMoon } from "@tabler/icons-react";
import { UI_THEMES } from "../lib/uiTheme.js";

const THEME_ICONS = {
  light: IconSun,
  system: IconSunMoon,
  dark: IconMoon,
};

function ThemeMenu({ theme, onThemeChange }) {
  const CurrentIcon = THEME_ICONS[theme] ?? IconSunMoon;
  const currentLabel =
    UI_THEMES.find((entry) => entry.value === theme)?.label ?? "System";

  return (
    <Menu position="bottom-end" withArrow>
      <Menu.Target>
        <ActionIcon
          variant="subtle"
          color="gray"
          size="lg"
          aria-label={`Color theme: ${currentLabel}`}
        >
          <CurrentIcon size={18} />
        </ActionIcon>
      </Menu.Target>
      <Menu.Dropdown>
        {UI_THEMES.map(({ value, label }) => {
          const Icon = THEME_ICONS[value] ?? IconSunMoon;
          return (
            <Menu.Item
              key={value}
              leftSection={<Icon size={16} />}
              onClick={() => onThemeChange(value)}
            >
              {label}
            </Menu.Item>
          );
        })}
      </Menu.Dropdown>
    </Menu>
  );
}

export default ThemeMenu;
