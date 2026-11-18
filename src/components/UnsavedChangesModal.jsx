// The unsaved-changes prompt (M9 task 3), shown when the dirty guard holds a
// Close/New/Open request. It is a real modal — the panes are blurred and
// `inert` while it is open (App.jsx) — but it is not a system dialog, so the
// window-close event never blocks and the same component works headlessly.
//
// The body is exported separately: component tests render it through
// react-dom/server, which cannot mount Mantine's portal-based Modal.
import { Button, Group, Modal, Stack, Text } from "@mantine/core";
import {
  CHOICE_CANCEL,
  CHOICE_DISCARD,
  CHOICE_SAVE,
} from "../lib/unsavedGuard.js";

export function UnsavedChangesBody({ busy, onChoose }) {
  return (
    <Stack gap="md">
      <Text size="sm">
        This document has unsaved changes. Save them before continuing?
      </Text>
      <Group justify="flex-end" gap="xs">
        <Button
          variant="default"
          size="compact-sm"
          disabled={busy}
          onClick={() => onChoose(CHOICE_CANCEL)}
        >
          Cancel
        </Button>
        <Button
          variant="default"
          size="compact-sm"
          disabled={busy}
          onClick={() => onChoose(CHOICE_DISCARD)}
        >
          Discard
        </Button>
        <Button
          size="compact-sm"
          color="blue"
          loading={busy}
          onClick={() => onChoose(CHOICE_SAVE)}
        >
          Save
        </Button>
      </Group>
    </Stack>
  );
}

/** `busy` while a Save is in flight (it may open the Save As dialog): every
 *  choice is disabled so the request cannot be settled twice. */
function UnsavedChangesModal({ open, busy, onChoose }) {
  return (
    <Modal
      opened={open}
      onClose={() => {
        if (!busy) onChoose(CHOICE_CANCEL);
      }}
      title="Unsaved changes"
      centered
      withOverlay={false}
      closeOnClickOutside={false}
      transitionProps={{ duration: 0 }}
    >
      <UnsavedChangesBody busy={busy} onChoose={onChoose} />
    </Modal>
  );
}

export default UnsavedChangesModal;
