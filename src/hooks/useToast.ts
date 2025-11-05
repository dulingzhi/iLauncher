import { create } from 'zustand';
import { ToastType } from '../components/Toast';

interface ToastState {
  message: string;
  type: ToastType;
  visible: boolean;
  showToast: (message: string, type?: ToastType) => void;
  hideToast: () => void;
}

export const useToast = create<ToastState>((set) => ({
  message: '',
  type: 'info',
  visible: false,
  showToast: (message: string, type: ToastType = 'info') => {
    set({ message, type, visible: true });
  },
  hideToast: () => {
    set({ visible: false });
  },
}));
