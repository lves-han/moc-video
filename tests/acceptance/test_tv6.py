"""Independent rejection tests for TV6's observable behavior validators."""
import copy
import unittest
import tv6


class Oracles(unittest.TestCase):
    def reject(self, validate, valid, mutate):
        validate(valid)
        invalid=copy.deepcopy(valid); mutate(invalid)
        with self.assertRaises((ValueError,AssertionError)):
            validate(invalid)

    def test_resume_pause_requires_settled_play_and_no_visible_error(self):
        row=dict(paused=True,phase='paused',error=None,errors=[],plays=[dict(state='resolved')])
        for mutate in [lambda value:value.update(paused=False),lambda value:value.update(phase='failed'),
                       lambda value:value.update(error=dict(code='UPSTREAM_FAILED')),lambda value:value['plays'][0].update(state='pending'),
                       lambda value:value['plays'][0].update(state='rejected'),lambda value:value.update(plays=[])]:
            self.reject(tv6.validate_resume_pause,row,mutate)

    def test_loading_must_identify_and_decode_blurred_backdrop(self):
        row=dict(phase='resolving',title='TV6 自有测试影片',episode='第1集',backdrop=dict(filter='blur(40px) brightness(0.35)'),titleStyle=dict(fontSize='28px',fontWeight='700'),episodeStyle=dict(fontSize='15px'),spinner=dict(width='28px',height='28px'))
        self.reject(tv6.validate_loading,row,lambda value:value['backdrop'].update(filter='none'))
        self.reject(tv6.validate_loading,row,lambda value:value.update(title=''))

    def test_controls_must_seek_exactly_ten_and_use_real_actions(self):
        row=dict(forward=22,backward=12,shown=True,off=True,speeds=[.5,1,1.25,1.5,2],volume=.5,registered=True,owner=1,errors=[])
        keys=dict(before=dict(position=12,volume=.5),right=dict(position=22),left=dict(position=12),up=dict(volume=.55),down=dict(volume=.5),mute=dict(muted=True),unmute=dict(muted=False),play=dict(paused=False),pause=dict(paused=True),fullscreen=dict(fullscreen=True),windowed=dict(fullscreen=False))
        self.reject(lambda value:tv6.validate_controls(value,keys),row,lambda value:value.update(forward=17))
        self.reject(lambda value:tv6.validate_controls(row,value),keys,lambda value:value['fullscreen'].update(fullscreen=False))

    def test_next_requires_real_eos_cancel_and_epoch_safety(self):
        row=dict(before=dict(next=False),visible=dict(next=True),offEos=dict(trusted=True),off=dict(phase='ended',countdown=None,matches=2),offMatches=2,
                 initialCountdown=dict(countdown='5',video='v0'),cancelledEos=dict(trusted=True),cancelled=dict(phase='ended',countdown=None,video='v0'),
                 autoEos=dict(trusted=True),advanced=dict(video='v1',elapsed=5010,phase='playing'),stale=dict(matches=5,phase='playing',countdown=None),staleCount=5,completedSaves=1,
                 card=dict(rect=dict(width=300),borderRadius='16px'),image=dict(rect=dict(width=300,height=168.75)),kicker=dict(fontSize='12px',fontWeight='600'),title=dict(fontSize='14px',fontWeight='500'))
        for mutate in [lambda value:value['autoEos'].update(trusted=False),lambda value:value['advanced'].update(elapsed=3000),lambda value:value['cancelled'].update(video='v1'),lambda value:value['stale'].update(matches=6),lambda value:value.update(completedSaves=0)]:
            self.reject(tv6.validate_next,row,mutate)

    def test_visual_requires_window_geometry_and_shared_glass(self):
        styles={selector:{} for selector in ['.playback-view','video','.playback-shade','.playback-header','.playback-back','.playback-header h2','.playback-header p','.playback-bar','.playback-track','media-slider-buffer','media-slider-fill','media-slider-thumb','.playback-preview','media-time','.playback-button','media-play-button media-icon','.playback-tray','.playback-now','.playback-buffering .playback-spinner']}
        styles['.playback-view']=dict(backgroundColor='rgb(0, 0, 0)',rect=dict(x=0,y=0,width=1200,height=800))
        styles['video']=dict(objectFit='contain',rect=dict(x=0,y=0,width=1200,height=800))
        styles['.playback-shade']=dict(rect=dict(height=240))
        styles['.playback-header']=dict(rect=dict(x=24,y=52))
        styles['.playback-back']=dict(rect=dict(width=44,height=44),borderRadius='50%')
        styles['.playback-header h2']=dict(fontSize='17px',fontWeight='600')
        styles['.playback-header p']=dict(fontSize='13px',fontWeight='400',color='rgba(235, 235, 245, 0.62)')
        styles['.playback-bar']=dict(rect=dict(x=24,y=666,width=1152,height=110),borderRadius='20px',paddingTop='12px',paddingRight='20px',paddingBottom='12px',paddingLeft='20px',backgroundColor='rgba(30, 30, 34, 0.62)',webkitBackdropFilter='blur(30px) saturate(1.6)')
        styles['.playback-track']=dict(rect=dict(height=4),backgroundColor='rgba(255, 255, 255, 0.3)')
        styles['media-slider-buffer']=dict(backgroundColor='rgba(255, 255, 255, 0.5)')
        styles['media-slider-fill']=dict(backgroundColor='rgb(245, 245, 247)')
        styles['media-slider-thumb']=dict(rect=dict(width=14,height=14))
        styles['.playback-preview']=dict(fontSize='12px',borderRadius='999px')
        styles['media-time']=dict(fontSize='13px',fontVariantNumeric='tabular-nums')
        styles['.playback-button']=dict(rect=dict(width=44,height=44))
        styles['media-play-button media-icon']=dict(rect=dict(width=28))
        styles['.playback-tray']=dict(rect=dict(height=368),borderTopLeftRadius='20px',borderTopRightRadius='20px',transitionDuration='0.3s, 0s')
        styles['.playback-now']=dict(rect=dict(width=1,height=1))
        styles['.playback-buffering .playback-spinner']=dict(width='40px',height='40px')
        row=dict(width=1200,height=800,styles=styles,bufferDelay=300,videoCount=1,controlsCount=1,legacy=False,errors=[],cue=dict(fontSize='28px',color='rgb(255, 255, 255)',textShadow='rgb(0, 0, 0) 2px 2px 0px'))
        for mutate in [lambda value:value.update(controlsCount=2),lambda value:value['styles']['video']['rect'].update(width=1000),lambda value:value['styles']['.playback-bar'].update(borderRadius='12px'),lambda value:value['styles']['.playback-header']['rect'].update(y=24),lambda value:value['cue'].update(fontSize='20px')]:
            self.reject(tv6.validate_styles,row,mutate)


if __name__=='__main__':
    unittest.main()
