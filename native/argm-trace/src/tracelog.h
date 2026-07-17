// argm-trace: thread-safe buffered trace log writer.
//
// The log records every hooked engine event (AI reinforcement jobs, party
// state changes, NPC activation, optional full BCI opcode stream) in a single
// append-only text file next to the game executable.
#pragma once

#include <cstdarg>
#include <cstdint>

namespace argm {

// Opens the log file (created/truncated once per process) and starts the
// background flush. Returns false if the file cannot be created; the rest of
// the DLL then stays completely passive.
bool LogOpen(const wchar_t* directory);

// Closes and flushes the log. Safe to call more than once.
void LogClose();

// Appends one formatted line. A timestamp and thread id are prepended and a
// newline is appended automatically. Never throws; drops the line if the log
// is not open.
void LogLine(const char* category, const char* fmt, ...);
void LogLineV(const char* category, const char* fmt, va_list args);

// Raw line with no category column (used for the header banner).
void LogRaw(const char* fmt, ...);

}  // namespace argm
