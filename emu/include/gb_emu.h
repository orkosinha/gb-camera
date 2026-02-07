#ifndef GB_EMU_H
#define GB_EMU_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

// Button constants
#define GB_BUTTON_A 0
#define GB_BUTTON_B 1
#define GB_BUTTON_SELECT 2
#define GB_BUTTON_START 3
#define GB_BUTTON_RIGHT 4
#define GB_BUTTON_LEFT 5
#define GB_BUTTON_UP 6
#define GB_BUTTON_DOWN 7

// Screen dimensions
#define GB_SCREEN_WIDTH 160
#define GB_SCREEN_HEIGHT 144

// Camera dimensions
#define GB_CAMERA_WIDTH 128
#define GB_CAMERA_HEIGHT 112

// Opaque handle to GameBoy emulator instance
typedef void* GBHandle;

// Lifecycle
GBHandle gb_create(void);
void gb_destroy(GBHandle handle);

// ROM loading
bool gb_load_rom(GBHandle handle, const uint8_t* data, size_t len);

// Emulation
void gb_step_frame(GBHandle handle);
uint32_t gb_get_frame_count(GBHandle handle);

// Frame buffer
const uint8_t* gb_get_frame_buffer(GBHandle handle);
size_t gb_get_frame_buffer_size(void);
uint32_t gb_get_screen_width(void);
uint32_t gb_get_screen_height(void);

// Input
void gb_set_button(GBHandle handle, uint8_t button, bool pressed);

// Camera
void gb_set_camera_image(GBHandle handle, const uint8_t* data, size_t len);
bool gb_is_camera_cartridge(GBHandle handle);
bool gb_is_camera_ready(GBHandle handle);
bool gb_update_camera_live(GBHandle handle);
const uint8_t* gb_camera_live_ptr(GBHandle handle);
size_t gb_camera_live_len(void);
size_t gb_decode_camera_photo(GBHandle handle, uint8_t slot, uint8_t* buffer, size_t buffer_len);
int32_t gb_camera_contrast(GBHandle handle);
void gb_set_camera_exposure(GBHandle handle, int32_t exposure);
bool gb_encode_camera_photo(GBHandle handle, uint8_t slot, const uint8_t* rgba, size_t len);
void gb_clear_camera_photo_slot(GBHandle handle, uint8_t slot);
uint8_t gb_camera_photo_count(GBHandle handle);

// Memory
uint8_t gb_read_memory(GBHandle handle, uint16_t addr);

// Save data
size_t gb_get_save_size(GBHandle handle);
size_t gb_get_save_data(GBHandle handle, uint8_t* buffer, size_t buffer_len);
bool gb_load_save_data(GBHandle handle, const uint8_t* data, size_t len);

#endif /* GB_EMU_H */
