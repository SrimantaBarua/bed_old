// (C) 2020 Srimanta Barua <srimanta.barua1@gmail.com>

use std::ffi::CStr;
use std::ops::Drop;
use std::rc::Rc;
use std::str;

use super::gl::{
    self,
    types::{GLenum, GLint, GLuint},
    GlInner,
};
use super::{Gl, Mat4};

/// An active shader program
pub(in crate::ui) struct ActiveShaderProgram<'a, 'b> {
    gl: &'a Gl,
    shader: &'b mut ShaderProgram,
}

impl<'a, 'b> ActiveShaderProgram<'a, 'b> {
    /// Set uniform matrix
    pub(in crate::ui) fn uniform_mat4f(&mut self, name: &CStr, mat: &Mat4) {
        unsafe {
            let loc = self
                .gl
                .gl
                .GetUniformLocation(self.shader.program, name.as_ptr());
            self.gl.gl.UniformMatrix4fv(loc, 1, gl::FALSE, mat.as_ptr());
        }
    }

    pub(in crate::ui) fn uniform_1i(&mut self, name: &CStr, i: GLint) {
        unsafe {
            let loc = self
                .gl
                .gl
                .GetUniformLocation(self.shader.program, name.as_ptr());
            self.gl.gl.Uniform1i(loc, i);
        }
    }
}

/// Handle to a shader program
pub(in crate::ui) struct ShaderProgram {
    gl: Rc<GlInner>,
    program: GLuint,
}

impl ShaderProgram {
    /// Compile and link a shader from the given vertex and fragment shader source
    pub(super) fn new(gl: Rc<GlInner>, vsrc: &str, fsrc: &str) -> Result<ShaderProgram, String> {
        let mut success = 1;
        let mut len = 0;
        let mut info_log = [0; 512];
        let vshdr = Shader::new(gl.clone(), vsrc, gl::VERTEX_SHADER, "vertex")?;
        let fshdr = Shader::new(gl.clone(), fsrc, gl::FRAGMENT_SHADER, "fragment")?;
        unsafe {
            let id = gl.CreateProgram();
            gl.AttachShader(id, vshdr.0);
            gl.AttachShader(id, fshdr.0);
            gl.LinkProgram(id);
            gl.GetProgramiv(id, gl::LINK_STATUS, &mut success);
            if success == 0 {
                gl.GetProgramInfoLog(id, 512, &mut len, info_log.as_mut_ptr() as *mut i8);
                let info_str = str::from_utf8(&info_log[..(len as usize)]).unwrap();
                Err(format!("failed to link shader program: {}", info_str))
            } else {
                Ok(ShaderProgram {
                    program: id,
                    gl: gl,
                })
            }
        }
    }

    /// Use shader program
    pub(super) fn use_program<'a, 'b>(&'b mut self, gl: &'a mut Gl) -> ActiveShaderProgram<'a, 'b> {
        unsafe {
            self.gl.UseProgram(self.program);
        }
        ActiveShaderProgram {
            gl: gl,
            shader: self,
        }
    }
}

impl Drop for ShaderProgram {
    fn drop(&mut self) {
        unsafe { self.gl.DeleteProgram(self.program) }
    }
}

/// Handle to an individual shader compilation unit
struct Shader(GLuint, Rc<GlInner>);

impl Shader {
    /// Compile shader from source
    fn new(gl: Rc<GlInner>, src: &str, typ: GLenum, name: &str) -> Result<Shader, String> {
        let mut success = 1;
        let mut len = 0;
        let mut info_log = [0; 512];
        unsafe {
            let id = gl.CreateShader(typ);
            gl.ShaderSource(id, 1, &(src.as_ptr() as *const i8), &(src.len() as i32));
            gl.CompileShader(id);
            gl.GetShaderiv(id, gl::COMPILE_STATUS, &mut success);
            if success == 0 {
                gl.GetShaderInfoLog(id, 512, &mut len, info_log.as_mut_ptr() as *mut i8);
                let info_str = str::from_utf8(&info_log[..(len as usize)]).unwrap();
                Err(format!("failed to compile {} shader: {}", name, info_str))
            } else {
                Ok(Shader(id, gl))
            }
        }
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe { self.1.DeleteShader(self.0) }
    }
}
