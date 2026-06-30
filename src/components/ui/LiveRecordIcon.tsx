export function LiveRecordIcon() {
  return (
    <div className="flex items-center justify-center">
      <svg
        width="200"
        height="200"
        viewBox="0 0 200 200"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        {/* Background circle with gradient */}
        <defs>
          <linearGradient id="bgGradient" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#FF6B6B" />
            <stop offset="100%" stopColor="#FF3838" />
          </linearGradient>
          <linearGradient id="liveGradient" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#FF6B6B" />
            <stop offset="100%" stopColor="#FF3838" />
          </linearGradient>
        </defs>
        
        {/* Main background circle */}
        <circle cx="100" cy="100" r="90" fill="url(#bgGradient)" />
        
        {/* Camera body */}
        <g>
          {/* Camera main body */}
          <rect
            x="50"
            y="75"
            width="80"
            height="50"
            rx="8"
            fill="white"
            opacity="0.95"
          />
          
          {/* Camera lens */}
          <circle cx="75" cy="100" r="18" fill="#FF3838" opacity="0.2" />
          <circle cx="75" cy="100" r="12" fill="#FF3838" opacity="0.3" />
          <circle cx="75" cy="100" r="8" fill="#FF3838" />
          
          {/* Lens reflection */}
          <circle cx="72" cy="97" r="3" fill="white" opacity="0.6" />
          
          {/* Record button indicator */}
          <circle cx="110" cy="90" r="8" fill="#FF3838">
            <animate
              attributeName="opacity"
              values="1;0.3;1"
              dur="1.5s"
              repeatCount="indefinite"
            />
          </circle>
          
          {/* Viewfinder lines */}
          <line x1="110" y1="105" x2="120" y2="105" stroke="white" strokeWidth="2" opacity="0.5" />
          <line x1="110" y1="110" x2="125" y2="110" stroke="white" strokeWidth="2" opacity="0.5" />
          <line x1="110" y1="115" x2="120" y2="115" stroke="white" strokeWidth="2" opacity="0.5" />
        </g>
        
        {/* LIVE indicator with pulse effect */}
        <g>
          <circle cx="145" cy="60" r="20" fill="white" opacity="0.2">
            <animate
              attributeName="r"
              values="20;25;20"
              dur="2s"
              repeatCount="indefinite"
            />
            <animate
              attributeName="opacity"
              values="0.2;0;0.2"
              dur="2s"
              repeatCount="indefinite"
            />
          </circle>
          <circle cx="145" cy="60" r="16" fill="white" />
          <circle cx="145" cy="60" r="8" fill="#FF3838">
            <animate
              attributeName="opacity"
              values="1;0.5;1"
              dur="1s"
              repeatCount="indefinite"
            />
          </circle>
        </g>
        
        {/* Bottom text: REC */}
        <text
          x="100"
          y="155"
          textAnchor="middle"
          fill="white"
          fontSize="16"
          fontWeight="bold"
          fontFamily="Arial, sans-serif"
          letterSpacing="2"
        >
          REC
        </text>
      </svg>
    </div>
  );
}
