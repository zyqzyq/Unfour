import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription,
  DialogBody, DialogFooter, DialogXClose, Button,
} from '@unfour/ui';

export const ConfirmDelete = () => (
  <Dialog defaultOpen>
    <DialogContent title="Delete Item">
      <DialogHeader>
        <DialogTitle>Delete Connection</DialogTitle>
        <DialogXClose />
      </DialogHeader>
      <DialogBody>
        <DialogDescription>
          Are you sure you want to delete this database connection? This action cannot be undone.
        </DialogDescription>
      </DialogBody>
      <DialogFooter>
        <Button variant="secondary">Cancel</Button>
        <Button>Delete</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
);
