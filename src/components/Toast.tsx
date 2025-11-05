import { useEffect } from 'react';
import { CheckCircle2, XCircle, Info, AlertTriangle } from 'lucide-react';

export type ToastType = 'success' | 'error' | 'info' | 'warning';

interface ToastProps {
  message: string;
  type?: ToastType;
  duration?: number;
  onClose: () => void;
}

export function Toast({ message, type = 'info', duration = 3000, onClose }: ToastProps) {
  useEffect(() => {
    const timer = setTimeout(() => {
      onClose();
    }, duration);

    return () => clearTimeout(timer);
  }, [duration, onClose]);

  const icons = {
    success: <CheckCircle2 className="w-5 h-5" />,
    error: <XCircle className="w-5 h-5" />,
    info: <Info className="w-5 h-5" />,
    warning: <AlertTriangle className="w-5 h-5" />
  };

  const colors = {
    success: {
      bg: 'bg-green-500/90',
      border: 'border-green-400',
      text: 'text-white'
    },
    error: {
      bg: 'bg-red-500/90',
      border: 'border-red-400',
      text: 'text-white'
    },
    info: {
      bg: 'bg-blue-500/90',
      border: 'border-blue-400',
      text: 'text-white'
    },
    warning: {
      bg: 'bg-yellow-500/90',
      border: 'border-yellow-400',
      text: 'text-white'
    }
  };

  const style = colors[type];

  return (
    <div
      className={`fixed top-6 left-1/2 transform -translate-x-1/2 z-50 
        flex items-center gap-3 px-4 py-3 rounded-lg shadow-2xl border
        ${style.bg} ${style.border} ${style.text}
        animate-in slide-in-from-top duration-300`}
      style={{ minWidth: '300px', maxWidth: '500px' }}
    >
      <div className="flex-shrink-0">
        {icons[type]}
      </div>
      <div className="flex-1 text-sm font-medium">
        {message}
      </div>
    </div>
  );
}
