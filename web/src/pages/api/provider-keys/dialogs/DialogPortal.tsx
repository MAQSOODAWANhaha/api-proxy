import React from 'react'
import { DialogType, LocalProviderKey, ProviderKeyEditFormState, ProviderKeyFormState } from '../types'
import AddDialog from './AddDialog'
import DeleteDialog from './DeleteDialog'
import EditDialog from './EditDialog'
import StatsDialog from './StatsDialog'

/** 对话框门户组件 */
const DialogPortal: React.FC<{
  type: DialogType
  selectedItem: LocalProviderKey | null
  onClose: () => void
  onAdd: (item: ProviderKeyFormState) => void
  onEdit: (item: ProviderKeyEditFormState) => void
  onDelete: () => void
  onRefresh: () => Promise<void>
}> = ({ type, selectedItem, onClose, onAdd, onEdit, onDelete }) => {
  if (!type) return null
  const editItem: ProviderKeyEditFormState | null = selectedItem
    ? {
        id: Number(selectedItem.id),
        provider: selectedItem.provider,
        provider_type_id: selectedItem.provider_type_id || 0,
        keyName: selectedItem.keyName,
        keyValue: selectedItem.keyValue,
        auth_type: selectedItem.auth_type || 'api_key',
        weight: selectedItem.weight,
        requestLimitPerMinute: selectedItem.requestLimitPerMinute,
        tokenLimitPromptPerMinute: selectedItem.tokenLimitPromptPerMinute,
        requestLimitPerDay: selectedItem.requestLimitPerDay,
        status: selectedItem.status,
        project_id: selectedItem.project_id || '',
      }
    : null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      {type === 'add' && <AddDialog onClose={onClose} onSubmit={onAdd} />}
      {type === 'edit' && editItem && <EditDialog item={editItem} onClose={onClose} onSubmit={onEdit} />}
      {type === 'delete' && selectedItem && (
        <DeleteDialog item={selectedItem} onClose={onClose} onConfirm={onDelete} />
      )}
      {type === 'stats' && selectedItem && <StatsDialog item={selectedItem} onClose={onClose} />}
    </div>
  )
}

export default DialogPortal
