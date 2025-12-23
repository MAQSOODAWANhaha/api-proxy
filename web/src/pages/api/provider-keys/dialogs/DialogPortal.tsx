import React from 'react'
import { DialogType, LocalProviderKey } from '../types'
import AddDialog from './AddDialog'
import DeleteDialog from './DeleteDialog'
import EditDialog from './EditDialog'
import StatsDialog from './StatsDialog'

/** 对话框门户组件 */
const DialogPortal: React.FC<{
  type: DialogType
  selectedItem: LocalProviderKey | null
  onClose: () => void
  onAdd: (item: Omit<LocalProviderKey, 'id' | 'usage' | 'cost' | 'createdAt' | 'healthCheck'>) => void
  onEdit: (item: LocalProviderKey) => void
  onDelete: () => void
  onRefresh: () => Promise<void>
}> = ({ type, selectedItem, onClose, onAdd, onEdit, onDelete }) => {
  if (!type) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      {type === 'add' && <AddDialog onClose={onClose} onSubmit={onAdd} />}
      {type === 'edit' && selectedItem && <EditDialog item={selectedItem} onClose={onClose} onSubmit={onEdit} />}
      {type === 'delete' && selectedItem && (
        <DeleteDialog item={selectedItem} onClose={onClose} onConfirm={onDelete} />
      )}
      {type === 'stats' && selectedItem && <StatsDialog item={selectedItem} onClose={onClose} />}
    </div>
  )
}

export default DialogPortal
