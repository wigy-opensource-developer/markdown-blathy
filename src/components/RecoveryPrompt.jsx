// The crash-recovery prompt (M9 task 4), shown once at startup when a
// snapshot newer than the file on disk exists. Closing it (X / Escape) keeps
// the snapshot for the next launch; only Discard deletes it, and Restore
// leaves it in place until a successful save — so a second crash still
// recovers the same edits.
//
// The body is exported separately: component tests render it through
// react-dom/server, which cannot mount Mantine's portal-based Modal.
import { Button, Group, Modal, Stack, Text } from "@mantine/core";
import { fileBasename } from "../lib/title.js";

const formatSavedAt = (seconds) =>
  new Date(seconds * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });

export function RecoveryPromptBody({ snapshot, onRestore, onDiscard }) {
  const label = snapshot.path ? fileBasename(snapshot.path) : "Untitled";
  return (
    <Stack gap="md">
      <Text size="sm">
        MarkdownBláthy found unsaved changes for <strong>{label}</strong> from{" "}
        {formatSavedAt(snapshot.savedAt)}.
      </Text>
      <Text size="xs" c="dimmed">
        Restoring marks the document as unsaved; nothing is written until you
        save. Closing this prompt keeps the snapshot for the next launch.
      </Text>
      <Group justify="flex-end" gap="xs">
        <Button variant="default" size="compact-sm" onClick={onDiscard}>
          Discard
        </Button>
        <Button size="compact-sm" color="blue" onClick={onRestore}>
          Restore
        </Button>
      </Group>
    </Stack>
  );
}

function RecoveryPrompt({ snapshot, onRestore, onDiscard, onClose }) {
  return (
    <Modal
      opened={Boolean(snapshot)}
      onClose={onClose}
      title="Recover unsaved changes"
      centered
      withOverlay={false}
      closeOnClickOutside={false}
      transitionProps={{ duration: 0 }}
    >
      {snapshot && (
        <RecoveryPromptBody
          snapshot={snapshot}
          onRestore={onRestore}
          onDiscard={onDiscard}
        />
      )}
    </Modal>
  );
}

export default RecoveryPrompt;
