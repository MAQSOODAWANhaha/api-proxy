import React from 'react';
import { ApiKey, DialogType } from '../types';
import AddDialog from './AddDialog';
import DeleteDialog from './DeleteDialog';
import EditDialog from './EditDialog';
import StatsDialog from './StatsDialog';

/** 对话框门户组件 */
const DialogPortal: React.FC<{
  type: DialogType;
  selectedItem: ApiKey | null;
  onClose: () => void;
  onAdd: (
    item: Omit<ApiKey, 'id' | 'usage' | 'created_at' | 'last_used_at' | 'api_key'>
  ) => void;
  onEdit: (item: ApiKey) => void;
  onDelete: () => void;
}> = ({ type, selectedItem, onClose, onAdd, onEdit, onDelete }) => {
  if (!type) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      {type === 'add' && <AddDialog onClose={onClose} onSubmit={onAdd} />}
      {type === 'edit' && selectedItem && (
        <EditDialog item={selectedItem} onClose={onClose} onSubmit={onEdit} />
      )}
      {type === 'delete' && selectedItem && (
        <DeleteDialog item={selectedItem} onClose={onClose} onConfirm={onDelete} />
      )}
      {type === 'stats' && selectedItem && (
        <StatsDialog item={selectedItem} onClose={onClose} />
      )}
    </div>
  );
};

export default DialogPortal;
