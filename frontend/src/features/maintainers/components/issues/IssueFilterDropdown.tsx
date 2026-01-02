import { ChevronDown, Check } from 'lucide-react';

interface IssueFilterDropdownProps {
  value: string;
  onChange: (value: string) => void;
  isOpen: boolean;
  onToggle: () => void;
  onClose: () => void;
}

const filterOptions = ['All', 'Waiting for review', 'In progress', 'Stale'];

export function IssueFilterDropdown({ value, onChange, isOpen, onToggle, onClose }: IssueFilterDropdownProps) {
  const handleSelect = (option: string) => {
    onChange(option);
    onClose();
  };

  return (
    <div className="relative flex-1 z-50">
      <button 
        className="w-full flex items-center justify-between px-4 py-3 rounded-[14px] backdrop-blur-[25px] bg-white/[0.15] border border-white/25 hover:bg-white/[0.2] hover:border-[#c9983a]/30 transition-all"
        onClick={onToggle}
      >
        <span className="text-[14px] font-semibold text-[#2d2820]">{value}</span>
        <ChevronDown className={`w-4 h-4 text-[#7a6b5a] transition-transform ${isOpen ? 'rotate-180' : ''}`} />
      </button>
      
      {/* Dropdown Menu */}
      {isOpen && (
        <>
          {/* Backdrop to close dropdown */}
          <div 
            className="fixed inset-0 z-40" 
            onClick={onClose}
          />
          
          {/* Dropdown content */}
          <div className="absolute top-full left-0 right-0 mt-2 bg-[#d4c5b0] rounded-[20px] border-2 border-white/40 z-50 overflow-hidden">
            {/* Header */}
            <div className="px-6 py-5 border-b-2 border-white/30 bg-gradient-to-b from-white/10 to-transparent">
              <h3 className="text-[17px] font-bold text-[#2d2820]">DEFAULT</h3>
            </div>
            
            {/* Options */}
            <div className="py-3">
              {filterOptions.map((option) => (
                <button
                  key={option}
                  className="w-full px-6 py-3.5 flex items-center justify-between hover:bg-[#c9b8a0] transition-all group"
                  onClick={() => handleSelect(option)}
                >
                  <span className="text-[15px] font-bold text-[#2d2820] group-hover:text-[#c9983a] transition-colors">
                    {option}
                  </span>
                  {value === option && (
                    <Check className="w-5 h-5 text-[#c9983a]" strokeWidth={2.5} />
                  )}
                </button>
              ))}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
