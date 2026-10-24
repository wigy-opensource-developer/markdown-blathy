// UI appearance control (light / system / dark) for the toolbar (§8.6 of
// SPEC.md). Rendered as the same subtle Button as its neighbours (New / Open /
// Save / Page / Export) so the padding, height and blue tint match; the
// chevron marks it as a menu. The current scheme is shown by the icon, while
// the menu entries pair each icon with its label — a bare "System" would not
// tell most users this is UI theming.
import { Button, Menu } from "@mantine/core";
import {
  IconChevronDown,
  IconMoon,
  IconSun,
  IconSunMoon,
} from "@tabler/icons-react";
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
        {/* Matches the toolbar's subtle buttons; no Tooltip wrapper because
            Mantine cannot wrap a floating Menu target in a Tooltip. */}
        <Button
          variant="subtle"
          size="compact-md"
          px="xs"
          aria-label={`Color theme: ${currentLabel}`}
          title={`Color theme: ${currentLabel}`}
          leftSection={<CurrentIcon size={16} />}
          rightSection={<IconChevronDown size={12} />}
        />
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
